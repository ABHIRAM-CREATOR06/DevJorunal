use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use serde::Serialize;

use crate::{Error, Relationship, RelationshipType, Result};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TraversalHit {
    pub entry_id: String,
    pub depth: usize,
    pub from_id: String,
    pub relationship_id: String,
    pub relationship_type: RelationshipType,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GraphStats {
    pub nodes: usize,
    pub relationships: usize,
    pub by_type: BTreeMap<String, usize>,
    pub has_directed_cycle: bool,
}

/// Deterministic in-memory graph over entry IDs and persisted relationship records.
pub struct KnowledgeGraph {
    nodes: BTreeSet<String>,
    relationships: Vec<Relationship>,
}

impl KnowledgeGraph {
    pub fn new(
        nodes: impl IntoIterator<Item = String>,
        relationships: Vec<Relationship>,
    ) -> Result<Self> {
        let nodes: BTreeSet<_> = nodes.into_iter().collect();
        let mut edge_keys = HashSet::new();
        let mut relationship_ids = HashSet::new();
        for relationship in &relationships {
            if !relationship_ids.insert(&relationship.id) {
                return Err(Error::Invalid(format!(
                    "duplicate relationship id {}",
                    relationship.id
                )));
            }
            if !nodes.contains(&relationship.source_id) || !nodes.contains(&relationship.target_id)
            {
                return Err(Error::Invalid(format!(
                    "relationship {} references a missing entry",
                    relationship.id
                )));
            }
            if relationship.source_id == relationship.target_id {
                return Err(Error::Invalid(
                    "self-referencing relationships are not allowed".into(),
                ));
            }
            let (source, target) = canonical_endpoints(relationship);
            let key = (source.to_owned(), target.to_owned(), relationship.kind);
            if !edge_keys.insert(key) {
                return Err(Error::Invalid(format!(
                    "duplicate {} relationship",
                    relationship.kind.as_str()
                )));
            }
        }
        Ok(Self {
            nodes,
            relationships,
        })
    }

    pub fn nodes(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().map(String::as_str)
    }

    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }

    pub fn outgoing(
        &self,
        entry_id: &str,
        filter: Option<RelationshipType>,
    ) -> Result<Vec<Relationship>> {
        self.ensure_node(entry_id)?;
        Ok(self
            .relationships
            .iter()
            .filter(|edge| {
                edge.source_id == entry_id && filter.is_none_or(|kind| edge.kind == kind)
            })
            .cloned()
            .collect())
    }

    pub fn incoming(
        &self,
        entry_id: &str,
        filter: Option<RelationshipType>,
    ) -> Result<Vec<Relationship>> {
        self.ensure_node(entry_id)?;
        Ok(self
            .relationships
            .iter()
            .filter(|edge| {
                edge.target_id == entry_id && filter.is_none_or(|kind| edge.kind == kind)
            })
            .cloned()
            .collect())
    }

    pub fn implementations_of(&self, decision_id: &str) -> Result<Vec<String>> {
        Ok(self
            .incoming(decision_id, Some(RelationshipType::Implements))?
            .into_iter()
            .map(|edge| edge.source_id)
            .collect())
    }

    pub fn issues_caused_by(&self, component_id: &str) -> Result<Vec<String>> {
        Ok(self
            .incoming(component_id, Some(RelationshipType::CausedBy))?
            .into_iter()
            .map(|edge| edge.source_id)
            .collect())
    }

    pub fn superseding_decisions(&self, superseded_decision_id: &str) -> Result<Vec<String>> {
        Ok(self
            .incoming(superseded_decision_id, Some(RelationshipType::Supersedes))?
            .into_iter()
            .map(|edge| edge.source_id)
            .collect())
    }

    pub fn blocked_tasks(&self, blocking_task_id: &str) -> Result<Vec<String>> {
        Ok(self
            .outgoing(blocking_task_id, Some(RelationshipType::Blocks))?
            .into_iter()
            .map(|edge| edge.target_id)
            .collect())
    }

    pub fn blocking_tasks(&self, blocked_task_id: &str) -> Result<Vec<String>> {
        Ok(self
            .incoming(blocked_task_id, Some(RelationshipType::Blocks))?
            .into_iter()
            .map(|edge| edge.source_id)
            .collect())
    }

    pub fn sessions_for_issue(&self, issue_id: &str) -> Result<Vec<String>> {
        Ok(self
            .outgoing(issue_id, Some(RelationshipType::DiscoveredDuring))?
            .into_iter()
            .map(|edge| edge.target_id)
            .collect())
    }

    pub fn neighbors(
        &self,
        entry_id: &str,
        depth: usize,
        filter: Option<RelationshipType>,
    ) -> Result<Vec<TraversalHit>> {
        self.ensure_node(entry_id)?;
        if depth > 32 {
            return Err(Error::Invalid(
                "graph traversal depth cannot exceed 32".into(),
            ));
        }
        if depth == 0 {
            return Ok(Vec::new());
        }
        let mut visited = HashSet::from([entry_id.to_owned()]);
        let mut queue = VecDeque::from([(entry_id.to_owned(), 0usize)]);
        let mut result = Vec::new();
        while let Some((current, level)) = queue.pop_front() {
            if level == depth {
                continue;
            }
            let mut adjacent = Vec::new();
            for edge in &self.relationships {
                if filter.is_some_and(|kind| edge.kind != kind) {
                    continue;
                }
                if edge.source_id == current {
                    adjacent.push((edge.target_id.as_str(), edge));
                }
                if edge.target_id == current {
                    adjacent.push((edge.source_id.as_str(), edge));
                }
            }
            adjacent.sort_by_key(|(node, edge)| (edge.kind.as_str(), *node, edge.id.as_str()));
            for (next, edge) in adjacent {
                if visited.insert(next.to_owned()) {
                    result.push(TraversalHit {
                        entry_id: next.to_owned(),
                        depth: level + 1,
                        from_id: current.clone(),
                        relationship_id: edge.id.clone(),
                        relationship_type: edge.kind,
                    });
                    queue.push_back((next.to_owned(), level + 1));
                }
            }
        }
        Ok(result)
    }

    /// Finds a directed path; `relates_to` edges can be followed in either direction.
    pub fn path(&self, source_id: &str, target_id: &str) -> Result<Option<Vec<String>>> {
        self.ensure_node(source_id)?;
        self.ensure_node(target_id)?;
        if source_id == target_id {
            return Ok(Some(vec![source_id.to_owned()]));
        }
        let mut previous = HashMap::from([(source_id.to_owned(), None::<String>)]);
        let mut queue = VecDeque::from([source_id.to_owned()]);
        while let Some(current) = queue.pop_front() {
            let mut next_nodes = Vec::new();
            for edge in &self.relationships {
                if edge.source_id == current {
                    next_nodes.push(edge.target_id.as_str());
                } else if edge.kind.is_symmetric() && edge.target_id == current {
                    next_nodes.push(edge.source_id.as_str());
                }
            }
            next_nodes.sort_unstable();
            next_nodes.dedup();
            for next in next_nodes {
                if previous.contains_key(next) {
                    continue;
                }
                previous.insert(next.to_owned(), Some(current.clone()));
                if next == target_id {
                    let mut path = vec![target_id.to_owned()];
                    let mut cursor = target_id;
                    while let Some(Some(parent)) = previous.get(cursor) {
                        path.push(parent.clone());
                        cursor = path.last().expect("path has parent");
                    }
                    path.reverse();
                    return Ok(Some(path));
                }
                queue.push_back(next.to_owned());
            }
        }
        Ok(None)
    }

    pub fn has_directed_cycle(&self) -> bool {
        let mut state = HashMap::<&str, u8>::new();
        for node in &self.nodes {
            if state.get(node.as_str()).copied().unwrap_or(0) == 0 && self.visit(node, &mut state) {
                return true;
            }
        }
        false
    }

    pub fn stats(&self) -> GraphStats {
        let mut by_type = BTreeMap::new();
        for edge in &self.relationships {
            *by_type.entry(edge.kind.as_str().to_owned()).or_insert(0) += 1;
        }
        GraphStats {
            nodes: self.nodes.len(),
            relationships: self.relationships.len(),
            by_type,
            has_directed_cycle: self.has_directed_cycle(),
        }
    }

    fn ensure_node(&self, id: &str) -> Result<()> {
        if self.nodes.contains(id) {
            Ok(())
        } else {
            Err(Error::NotFound(id.to_owned()))
        }
    }

    fn visit<'a>(&'a self, node: &'a str, state: &mut HashMap<&'a str, u8>) -> bool {
        state.insert(node, 1);
        for edge in self
            .relationships
            .iter()
            .filter(|edge| edge.source_id == node && !edge.kind.is_symmetric())
        {
            let target = edge.target_id.as_str();
            match state.get(target).copied().unwrap_or(0) {
                1 => return true,
                0 if self.visit(target, state) => return true,
                _ => {}
            }
        }
        state.insert(node, 2);
        false
    }
}

fn canonical_endpoints(edge: &Relationship) -> (&str, &str) {
    if edge.kind.is_symmetric() && edge.source_id > edge.target_id {
        (&edge.target_id, &edge.source_id)
    } else {
        (&edge.source_id, &edge.target_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Author, AuthorType, EntrySource, EntrySourceType};

    fn edge(source: &str, target: &str, kind: RelationshipType) -> Relationship {
        Relationship::new(
            source,
            target,
            kind,
            Author {
                kind: AuthorType::Ai,
                name: "agent".into(),
            },
            EntrySource {
                kind: EntrySourceType::Agent,
                detail: Some("automated relationship".into()),
            },
        )
    }

    #[test]
    fn traverses_neighbors_by_depth_and_respects_direction_for_paths() {
        let graph = KnowledgeGraph::new(
            ["dj-a", "dj-b", "dj-c"].map(str::to_owned),
            vec![
                edge("dj-a", "dj-b", RelationshipType::Implements),
                edge("dj-b", "dj-c", RelationshipType::DependsOn),
            ],
        )
        .unwrap();
        assert_eq!(graph.outgoing("dj-a", None).unwrap().len(), 1);
        assert_eq!(
            graph
                .incoming("dj-b", Some(RelationshipType::Implements))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            graph
                .neighbors("dj-a", 1, None)
                .unwrap()
                .iter()
                .map(|hit| hit.entry_id.as_str())
                .collect::<Vec<_>>(),
            vec!["dj-b"]
        );
        assert_eq!(graph.neighbors("dj-a", 2, None).unwrap().len(), 2);
        assert_eq!(
            graph.path("dj-a", "dj-c").unwrap(),
            Some(vec!["dj-a".into(), "dj-b".into(), "dj-c".into()])
        );
        assert_eq!(graph.path("dj-c", "dj-a").unwrap(), None);
        assert!(graph.neighbors("dj-a", 33, None).is_err());
    }

    #[test]
    fn symmetric_duplicates_and_cycles_are_handled() {
        let duplicate = KnowledgeGraph::new(
            ["dj-a", "dj-b"].map(str::to_owned),
            vec![
                edge("dj-a", "dj-b", RelationshipType::RelatesTo),
                edge("dj-b", "dj-a", RelationshipType::RelatesTo),
            ],
        );
        assert!(duplicate.is_err());

        let cyclic = KnowledgeGraph::new(
            ["dj-a", "dj-b", "dj-c"].map(str::to_owned),
            vec![
                edge("dj-a", "dj-b", RelationshipType::DependsOn),
                edge("dj-b", "dj-c", RelationshipType::DependsOn),
                edge("dj-c", "dj-a", RelationshipType::DependsOn),
            ],
        )
        .unwrap();
        assert!(cyclic.has_directed_cycle());
    }

    #[test]
    fn rejects_missing_nodes_and_self_reference() {
        assert!(
            KnowledgeGraph::new(
                ["dj-a".into()],
                vec![edge("dj-a", "dj-missing", RelationshipType::References)]
            )
            .is_err()
        );
        assert!(
            edge("dj-a", "dj-a", RelationshipType::References)
                .validate()
                .is_err()
        );
    }
}
