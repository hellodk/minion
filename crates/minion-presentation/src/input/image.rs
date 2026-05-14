use anyhow::bail;
use minion_llm::LlmProvider;
pub async fn process_image(_path: &str, _llm: &dyn LlmProvider) -> anyhow::Result<String> {
    bail!("image processor not yet implemented — see Task 5");
}
