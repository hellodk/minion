#[test]
fn orchestrator_type_exists() {
    let _ = std::mem::size_of::<minion_presentation::orchestrator::Orchestrator>();
}
