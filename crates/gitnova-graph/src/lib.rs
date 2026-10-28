use gitnova_domain::*;
/// Reserve the checked-out revision's column without drawing an invented edge above its tip.
pub fn reserve_head(state: &mut GraphCursor, oid: String) {
    if state.lanes.is_empty() {
        state.lanes.push(Some(GraphLane {
            oid,
            color: state.next_color,
            active: false,
        }));
        state.next_color += 1;
    }
}
fn vacant(state: &GraphCursor, start: usize) -> usize {
    state
        .lanes
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, lane)| lane.is_none())
        .map_or(state.lanes.len(), |(index, _)| index)
}
fn put(state: &mut GraphCursor, index: usize, lane: GraphLane) {
    if index == state.lanes.len() {
        state.lanes.push(Some(lane));
    } else {
        state.lanes[index] = Some(lane);
    }
}
fn find(state: &GraphCursor, oid: &str) -> Option<usize> {
    state
        .lanes
        .iter()
        .position(|lane| lane.as_ref().is_some_and(|lane| lane.oid == oid))
}
/// Keep active columns and colors stable. Empty middle columns are reusable slots, not a reason to shift live lines.
pub fn layout(commits: Vec<Commit>, state: &mut GraphCursor) -> Vec<GraphRow> {
    commits
        .into_iter()
        .map(|commit| {
            let incoming: Vec<_> = state
                .lanes
                .iter()
                .enumerate()
                .filter_map(|(lane, entry)| {
                    entry
                        .as_ref()
                        .filter(|entry| entry.active)
                        .map(|entry| LaneStroke {
                            lane,
                            color: entry.color,
                        })
                })
                .collect();
            let before = state.lanes.clone();
            let lane = find(state, &commit.oid).unwrap_or_else(|| {
                let lane = vacant(state, 0);
                let color = state.next_color;
                state.next_color += 1;
                put(
                    state,
                    lane,
                    GraphLane {
                        oid: commit.oid.clone(),
                        color,
                        active: false,
                    },
                );
                lane
            });
            let color = state.lanes[lane]
                .take()
                .expect("current lane was assigned")
                .color;
            let mut parents = vec![];
            for (index, parent) in commit.parents.iter().enumerate() {
                if parents.iter().any(|(oid, _, _)| oid == parent) {
                    continue;
                }
                let (target, edge_color) = if let Some(target) = find(state, parent) {
                    state.lanes[target]
                        .as_mut()
                        .expect("matched lane exists")
                        .active = true;
                    (target, color)
                } else {
                    let target = if index == 0 {
                        lane
                    } else {
                        vacant(state, lane + 1)
                    };
                    let edge_color = if index == 0 {
                        color
                    } else {
                        let next = state.next_color;
                        state.next_color += 1;
                        next
                    };
                    put(
                        state,
                        target,
                        GraphLane {
                            oid: parent.clone(),
                            color: edge_color,
                            active: true,
                        },
                    );
                    (target, edge_color)
                };
                parents.push((parent.clone(), target, edge_color));
            }
            let mut edges = vec![];
            for (index, entry) in before.iter().enumerate() {
                if let Some(entry) = entry {
                    if entry.active && entry.oid != commit.oid {
                        edges.push(Edge {
                            from: index,
                            to: index,
                            color: entry.color,
                        });
                    }
                }
            }
            for (_, target, edge_color) in parents {
                edges.push(Edge {
                    from: lane,
                    to: target,
                    color: edge_color,
                });
            }
            let width = before.len().max(state.lanes.len()).max(lane + 1);
            while state.lanes.last().is_some_and(Option::is_none) {
                state.lanes.pop();
            }
            GraphRow {
                commit,
                lane,
                color,
                incoming,
                edges,
                width,
            }
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn c(oid: &str, parents: &[&str]) -> Commit {
        Commit {
            oid: oid.into(),
            parents: parents.iter().map(|p| (*p).into()).collect(),
            author: "A".into(),
            email: "a@b".into(),
            timestamp: 0,
            subject: oid.into(),
        }
    }
    fn signatures(rows: &[GraphRow]) -> Vec<String> {
        rows.iter()
            .map(|row| {
                format!(
                    "{}:{}:{}:{:?}:{:?}",
                    row.commit.oid,
                    row.lane,
                    row.color,
                    row.incoming
                        .iter()
                        .map(|x| (x.lane, x.color))
                        .collect::<Vec<_>>(),
                    row.edges
                        .iter()
                        .map(|e| (e.from, e.to, e.color))
                        .collect::<Vec<_>>()
                )
            })
            .collect()
    }
    #[test]
    fn pagination_preserves_positions_colors_and_octopus_edges() {
        let commits = vec![
            c("m", &["a", "b", "c"]),
            c("a", &["r"]),
            c("b", &["r"]),
            c("c", &["r"]),
            c("r", &[]),
            c("other", &[]),
        ];
        let all = layout(commits.clone(), &mut GraphCursor::default());
        for boundary in 1..commits.len() {
            let mut state = GraphCursor::default();
            let mut pages = layout(commits[..boundary].to_vec(), &mut state);
            pages.extend(layout(commits[boundary..].to_vec(), &mut state));
            assert_eq!(signatures(&all), signatures(&pages));
            assert!(state.lanes.is_empty());
        }
        assert_eq!(all[0].edges.len(), 3);
        assert!(all[0].incoming.is_empty());
    }
    #[test]
    fn finishing_left_lane_does_not_shift_or_recolor_right_lane() {
        let rows = layout(
            vec![
                c("left", &["left-root"]),
                c("right", &["right-parent"]),
                c("left-root", &[]),
                c("right-parent", &["right-root"]),
                c("right-root", &[]),
            ],
            &mut GraphCursor::default(),
        );
        assert_eq!(rows[1].lane, 1);
        assert_eq!(rows[3].lane, 1);
        assert_eq!(rows[1].color, rows[3].color);
        assert!(rows[3]
            .incoming
            .iter()
            .any(|stroke| stroke.lane == 1 && stroke.color == rows[1].color));
    }
    #[test]
    fn checked_out_lane_can_be_reserved_without_a_phantom_incoming_edge() {
        let mut state = GraphCursor::default();
        reserve_head(&mut state, "head".into());
        let rows = layout(
            vec![
                c("newer-tip", &["side-root"]),
                c("head", &["main-root"]),
                c("side-root", &[]),
                c("main-root", &[]),
            ],
            &mut state,
        );
        assert_eq!(rows[0].lane, 1);
        assert!(rows[0].incoming.is_empty());
        assert_eq!(rows[1].lane, 0);
        assert_eq!(rows[1].color, 0);
        assert!(!rows[1].incoming.iter().any(|stroke| stroke.lane == 0));
    }
    #[test]
    fn first_parent_keeps_color_and_new_tips_get_new_identities() {
        let rows = layout(
            vec![
                c("m", &["main", "side"]),
                c("main", &["root"]),
                c("side", &["root"]),
                c("root", &[]),
                c("independent", &[]),
            ],
            &mut GraphCursor::default(),
        );
        assert_eq!(rows[0].color, rows[1].color);
        assert_ne!(rows[1].color, rows[2].color);
        assert_ne!(rows[0].color, rows[4].color);
        assert_eq!(rows[2].edges.last().unwrap().color, rows[2].color);
        assert!(rows[4].incoming.is_empty());
    }
    #[test]
    fn criss_cross_histories_converge_without_losing_a_parent_edge() {
        let mut state = GraphCursor::default();
        let rows = layout(
            vec![
                c("x", &["a", "b"]),
                c("y", &["b", "a"]),
                c("a", &["r"]),
                c("b", &["r"]),
                c("r", &[]),
            ],
            &mut state,
        );
        assert_eq!(rows[1].edges.len(), 4);
        assert!(state.lanes.is_empty());
        assert_eq!(rows[4].lane, 0);
    }
}
