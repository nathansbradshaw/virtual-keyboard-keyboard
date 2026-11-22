use super::*;

fn reference(stages: &[Vec<KeyId>], positions: &HashMap<KeyId, (f32, f32)>) -> Vec<KeyId> {
    let distance = |a, b| {
        let (a, b) = (positions[&a], positions[&b]);
        ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
    };
    let mut rows = vec![vec![(0.0_f32, 0); stages[0].len()]];
    for i in 1..stages.len() {
        rows.push(
            stages[i]
                .iter()
                .map(|&key| {
                    stages[i - 1]
                        .iter()
                        .enumerate()
                        .map(|(j, &previous)| (rows[i - 1][j].0 + distance(previous, key), j))
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                        .unwrap()
                })
                .collect::<Vec<_>>(),
        );
    }
    let mut index = (0..rows.last().unwrap().len())
        .min_by(|&a, &b| {
            rows.last().unwrap()[a]
                .0
                .total_cmp(&rows.last().unwrap()[b].0)
        })
        .unwrap();
    let mut result = vec![stages[0][0]; stages.len()];
    for i in (0..stages.len()).rev() {
        result[i] = stages[i][index];
        index = rows[i][index].1;
    }
    result
}

#[test]
fn flat_traceback_preserves_paths_and_tie_breaking() {
    let positions = (0..20)
        .map(|i| (KeyId(i), ((i % 5) as f32, (i / 5) as f32)))
        .collect();
    for seed in 0..50 {
        let stages: Vec<Vec<_>> = (0..100)
            .map(|i| {
                (0..(1 + (i + seed) % 5))
                    .map(|j| KeyId((i * 7 + j * 3 + seed) % 20))
                    .collect()
            })
            .collect();
        assert_eq!(
            shortest_path_keys(&stages, &positions),
            reference(&stages, &positions)
        );
    }
    assert!(shortest_path_keys(&[], &positions).is_empty());
}

#[test]
fn preparation_rejects_an_already_cancelled_load() {
    let generation = work_budget::LoadGeneration::default();
    let ticket = generation.begin();
    generation.begin();
    let result = work_budget::complete_immediately(prepare_midi(
        Vec::new(),
        ticket,
        HashSet::new(),
        HashMap::new(),
        HashMap::new(),
    ));
    assert_eq!(result.unwrap_err(), "MIDI loading cancelled");
}
