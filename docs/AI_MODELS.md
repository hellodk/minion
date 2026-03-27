# MINION AI Model Recommendations

## Overview

MINION integrates AI capabilities through local LLM inference (Ollama) and efficient embedding models. This document outlines model recommendations per module and deployment strategy.

---

## AI Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      AI LAYER ARCHITECTURE                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌─────────────────────────────────────────────────────────┐  │
│   │                    MINION MODULES                        │  │
│   │  Media │ Files │ Blog │ Finance │ Fitness │ Reader      │  │
│   └─────────────────────────────────────────────────────────┘  │
│                              │                                  │
│                              ▼                                  │
│   ┌─────────────────────────────────────────────────────────┐  │
│   │                    AI ABSTRACTION                        │  │
│   │                                                          │  │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │  │
│   │  │   Prompt    │  │   Model     │  │  Response   │     │  │
│   │  │  Templates  │  │  Selector   │  │   Parser    │     │  │
│   │  └─────────────┘  └─────────────┘  └─────────────┘     │  │
│   │                                                          │  │
│   └─────────────────────────────────────────────────────────┘  │
│                              │                                  │
│         ┌────────────────────┼────────────────────┐            │
│         │                    │                    │            │
│         ▼                    ▼                    ▼            │
│   ┌───────────┐        ┌───────────┐        ┌───────────┐     │
│   │  Ollama   │        │   ONNX    │        │  Future   │     │
│   │  (LLM)    │        │ (Embed)   │        │ Providers │     │
│   └───────────┘        └───────────┘        └───────────┘     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Model Categories

### 1. Large Language Models (LLM)

For text generation, summarization, and reasoning tasks.

| Model | Size | VRAM | Speed | Quality | Use Case |
|-------|------|------|-------|---------|----------|
| **Llama 3.2 3B** | 2GB | 4GB | Fast | Good | Default, resource-constrained |
| **Llama 3.1 8B** | 5GB | 8GB | Medium | Very Good | Balanced performance |
| **Llama 3.1 70B** | 40GB | 48GB | Slow | Excellent | Complex reasoning |
| **Mistral 7B** | 4GB | 8GB | Fast | Very Good | Alternative default |
| **Mixtral 8x7B** | 26GB | 32GB | Medium | Excellent | Complex tasks |
| **Phi-3 Mini** | 2GB | 4GB | Very Fast | Good | Quick tasks |
| **Qwen2.5 7B** | 4GB | 8GB | Fast | Very Good | Multilingual |
| **CodeLlama 7B** | 4GB | 8GB | Fast | Excellent | Code generation |

### 2. Embedding Models

For semantic search and RAG.

| Model | Dimensions | Size | Quality | Use Case |
|-------|------------|------|---------|----------|
| **all-MiniLM-L6-v2** | 384 | 90MB | Good | Default, fast |
| **nomic-embed-text** | 768 | 270MB | Very Good | Balanced |
| **bge-base-en-v1.5** | 768 | 420MB | Excellent | High quality search |
| **e5-small-v2** | 384 | 130MB | Good | Alternative fast |
| **mxbai-embed-large** | 1024 | 670MB | Excellent | Maximum quality |

### 3. Vision Models

For image understanding and thumbnail analysis.

| Model | Size | VRAM | Use Case |
|-------|------|------|----------|
| **LLaVA 7B** | 4GB | 8GB | Image understanding |
| **BakLLaVA** | 4GB | 8GB | Visual reasoning |
| **moondream2** | 1.5GB | 4GB | Fast image analysis |

---

## Module-Specific Recommendations

### Module 1: Media Intelligence

```yaml
tasks:
  title_generation:
    model: llama3.2:3b
    fallback: phi3:mini
    prompt_type: creative
    temperature: 0.8
    max_tokens: 100
    
  description_generation:
    model: llama3.1:8b
    fallback: llama3.2:3b
    prompt_type: descriptive
    temperature: 0.7
    max_tokens: 500
    
  tag_generation:
    model: llama3.2:3b
    prompt_type: extraction
    temperature: 0.3
    max_tokens: 200
    
  thumbnail_analysis:
    model: moondream2
    prompt_type: visual
    
  seo_optimization:
    model: llama3.1:8b
    prompt_type: analytical
    temperature: 0.5
```

### Module 2: File Intelligence

```yaml
tasks:
  file_categorization:
    model: phi3:mini
    prompt_type: classification
    temperature: 0.2
    
  duplicate_naming:
    model: llama3.2:3b
    prompt_type: descriptive
    temperature: 0.3
    
  organization_suggestions:
    model: llama3.1:8b
    prompt_type: analytical
    temperature: 0.5

embeddings:
  file_metadata:
    model: all-MiniLM-L6-v2
    dimension: 384
```

### Module 3: Blog AI Engine

```yaml
tasks:
  topic_ideation:
    model: llama3.1:8b
    prompt_type: creative
    temperature: 0.9
    max_tokens: 500
    
  outline_generation:
    model: llama3.1:8b
    prompt_type: structured
    temperature: 0.6
    max_tokens: 1000
    
  content_writing:
    model: llama3.1:8b  # or 70b for best quality
    prompt_type: creative
    temperature: 0.7
    max_tokens: 2000
    
  seo_analysis:
    model: llama3.2:3b
    prompt_type: analytical
    temperature: 0.3
    max_tokens: 500
    
  keyword_clustering:
    model: llama3.2:3b
    prompt_type: extraction
    temperature: 0.2

embeddings:
  blog_content:
    model: bge-base-en-v1.5
    dimension: 768
```

### Module 4: Finance Intelligence

```yaml
tasks:
  transaction_categorization:
    model: phi3:mini
    prompt_type: classification
    temperature: 0.1  # Low for consistency
    max_tokens: 50
    
  spending_analysis:
    model: llama3.1:8b
    prompt_type: analytical
    temperature: 0.3
    max_tokens: 500
    
  investment_summary:
    model: llama3.1:8b
    prompt_type: analytical
    temperature: 0.3
    max_tokens: 1000
    
  anomaly_explanation:
    model: llama3.2:3b
    prompt_type: explanatory
    temperature: 0.4

# No embeddings needed - structured data queries
```

### Module 5: Fitness & Wellness

```yaml
tasks:
  motivational_quotes:
    model: llama3.2:3b
    prompt_type: creative
    temperature: 0.9
    max_tokens: 100
    
  workout_suggestions:
    model: llama3.1:8b
    prompt_type: instructional
    temperature: 0.5
    max_tokens: 500
    
  progress_analysis:
    model: llama3.2:3b
    prompt_type: analytical
    temperature: 0.3
    max_tokens: 300
    
  habit_recommendations:
    model: llama3.2:3b
    prompt_type: advisory
    temperature: 0.5

# No embeddings needed - structured data
```

### Module 6: Book Reader

```yaml
tasks:
  chapter_summary:
    model: llama3.1:8b
    fallback: llama3.2:3b
    prompt_type: summarization
    temperature: 0.4
    max_tokens: 500
    
  question_answering:
    model: llama3.1:8b
    prompt_type: rag_qa
    temperature: 0.3
    max_tokens: 1000
    
  concept_extraction:
    model: llama3.1:8b
    prompt_type: extraction
    temperature: 0.2
    max_tokens: 500
    
  timeline_extraction:
    model: llama3.1:8b
    prompt_type: structured_extraction
    temperature: 0.2
    max_tokens: 1000
    
  cross_book_synthesis:
    model: llama3.1:8b  # or 70b for complex analysis
    prompt_type: synthesis
    temperature: 0.4
    max_tokens: 2000

embeddings:
  book_chunks:
    model: bge-base-en-v1.5  # High quality for knowledge base
    dimension: 768
    chunk_size: 512
    chunk_overlap: 50
```

---

## Prompt Templates

### Summarization Template

```
You are a concise summarizer. Summarize the following text in {length} sentences.

TEXT:
{content}

SUMMARY:
```

### Classification Template

```
Classify the following into one of these categories: {categories}

TEXT: {text}

Return only the category name, nothing else.

CATEGORY:
```

### RAG Question Answering Template

```
Use the following context to answer the question. If the answer is not in the context, say "I don't have enough information to answer this question."

CONTEXT:
{context}

QUESTION: {question}

ANSWER:
```

### Creative Generation Template

```
You are a creative content writer. Generate {count} {content_type} for the following:

TOPIC: {topic}
STYLE: {style}
CONSTRAINTS: {constraints}

OUTPUT:
```

---

## Hardware Requirements

### Minimum (3B Models)
- **CPU**: 4 cores
- **RAM**: 8GB system, 4GB for inference
- **GPU**: Optional (CPU inference viable)
- **Storage**: 10GB for models

### Recommended (7-8B Models)
- **CPU**: 8 cores
- **RAM**: 16GB system
- **GPU**: 8GB VRAM (RTX 3070/4060 or equivalent)
- **Storage**: 20GB for models

### Optimal (Large Models)
- **CPU**: 16+ cores
- **RAM**: 32GB+ system
- **GPU**: 24GB+ VRAM (RTX 4090, A5000)
- **Storage**: 100GB for models

---

## Model Selection Logic

```rust
/// Automatic model selection based on task and available resources
pub struct ModelSelector {
    available_models: Vec<ModelInfo>,
    hardware_profile: HardwareProfile,
}

impl ModelSelector {
    pub fn select_model(&self, task: &AITask) -> ModelConfig {
        // Get task requirements
        let requirements = task.requirements();
        
        // Filter compatible models
        let compatible: Vec<_> = self.available_models.iter()
            .filter(|m| m.meets_requirements(&requirements))
            .filter(|m| self.hardware_profile.can_run(m))
            .collect();
        
        // Select best model for task
        compatible.into_iter()
            .max_by_key(|m| m.score_for_task(&task.task_type))
            .map(|m| m.to_config())
            .unwrap_or_else(|| self.fallback_config())
    }
}

/// Hardware detection for model compatibility
pub struct HardwareProfile {
    pub total_ram_gb: u32,
    pub available_ram_gb: u32,
    pub gpu_vram_gb: Option<u32>,
    pub cpu_cores: u32,
}

impl HardwareProfile {
    pub fn detect() -> Self {
        // Platform-specific hardware detection
        Self {
            total_ram_gb: sys_info::mem_info().map(|m| m.total / 1024 / 1024).unwrap_or(8) as u32,
            available_ram_gb: sys_info::mem_info().map(|m| m.avail / 1024 / 1024).unwrap_or(4) as u32,
            gpu_vram_gb: detect_gpu_vram(),
            cpu_cores: num_cpus::get() as u32,
        }
    }
    
    pub fn can_run(&self, model: &ModelInfo) -> bool {
        // Check if hardware can run the model
        if let Some(vram) = self.gpu_vram_gb {
            model.vram_required_gb <= vram
        } else {
            // CPU-only: need more RAM
            model.ram_required_gb <= self.available_ram_gb
        }
    }
}
```

---

## Ollama Integration

### Configuration

```toml
# ~/.minion/config/ai.toml

[ollama]
# Ollama server URL (default: localhost)
host = "127.0.0.1"
port = 11434

# Connection settings
timeout_seconds = 300
max_retries = 3

# Default model settings
default_model = "llama3.2:3b"
default_embedding_model = "nomic-embed-text"

[ollama.options]
# Context window
num_ctx = 4096

# Generation settings
num_predict = 1024
temperature = 0.7
top_p = 0.9
top_k = 40

# Performance
num_thread = 0  # Auto-detect
num_gpu = -1    # Use all GPUs

[models]
# Model-specific overrides
[models."llama3.1:8b"]
num_ctx = 8192

[models."llama3.1:70b"]
num_ctx = 4096  # Reduced for memory
```

### API Usage

```rust
/// Ollama client wrapper
pub struct OllamaClient {
    base_url: String,
    client: reqwest::Client,
    config: OllamaConfig,
}

impl OllamaClient {
    /// Generate completion
    pub async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse> {
        let url = format!("{}/api/generate", self.base_url);
        
        let response = self.client.post(&url)
            .json(&request)
            .timeout(Duration::from_secs(self.config.timeout_seconds))
            .send()
            .await?;
        
        // Handle streaming response
        if request.stream {
            self.handle_streaming_response(response).await
        } else {
            response.json().await.map_err(Into::into)
        }
    }
    
    /// Generate embeddings
    pub async fn embed(&self, request: EmbedRequest) -> Result<Vec<Vec<f32>>> {
        let url = format!("{}/api/embeddings", self.base_url);
        
        let response: EmbedResponse = self.client.post(&url)
            .json(&request)
            .send()
            .await?
            .json()
            .await?;
        
        Ok(response.embedding)
    }
    
    /// Check if model is available
    pub async fn check_model(&self, model: &str) -> Result<bool> {
        let url = format!("{}/api/tags", self.base_url);
        let response: TagsResponse = self.client.get(&url).send().await?.json().await?;
        
        Ok(response.models.iter().any(|m| m.name == model))
    }
    
    /// Pull model if not available
    pub async fn ensure_model(&self, model: &str) -> Result<()> {
        if !self.check_model(model).await? {
            self.pull_model(model).await?;
        }
        Ok(())
    }
}
```

---

## Embedding Pipeline

### Vector Store Integration

```rust
/// Vector store using usearch
pub struct VectorStore {
    index: usearch::Index,
    dimension: usize,
    metadata_db: Connection,
}

impl VectorStore {
    /// Add embeddings to the index
    pub fn add(&mut self, id: &str, embedding: &[f32], metadata: &Metadata) -> Result<()> {
        // Generate numeric key
        let key = self.next_key();
        
        // Add to usearch index
        self.index.add(key, embedding)?;
        
        // Store metadata mapping
        self.metadata_db.execute(
            "INSERT INTO vector_metadata (key, id, metadata) VALUES (?, ?, ?)",
            params![key, id, serde_json::to_string(metadata)?],
        )?;
        
        Ok(())
    }
    
    /// Search for similar vectors
    pub fn search(&self, query: &[f32], limit: usize) -> Result<Vec<SearchResult>> {
        let results = self.index.search(query, limit)?;
        
        // Fetch metadata for results
        let mut output = Vec::with_capacity(limit);
        for (key, distance) in results {
            let metadata = self.get_metadata(key)?;
            output.push(SearchResult {
                id: metadata.id,
                score: 1.0 - distance,  // Convert distance to similarity
                metadata: metadata.data,
            });
        }
        
        Ok(output)
    }
}
```

### RAG Pipeline

```rust
/// RAG (Retrieval Augmented Generation) pipeline
pub struct RAGPipeline {
    embedder: Box<dyn Embedder>,
    vector_store: VectorStore,
    llm: OllamaClient,
    config: RAGConfig,
}

impl RAGPipeline {
    /// Answer a question using RAG
    pub async fn answer(&self, question: &str, context_filter: Option<ContextFilter>) -> Result<RAGResponse> {
        // 1. Embed the question
        let query_embedding = self.embedder.embed(question).await?;
        
        // 2. Retrieve relevant chunks
        let results = self.vector_store.search(&query_embedding, self.config.top_k)?;
        
        // 3. Apply filter if provided
        let filtered = match context_filter {
            Some(filter) => results.into_iter().filter(|r| filter.matches(&r.metadata)).collect(),
            None => results,
        };
        
        // 4. Build context
        let context = self.build_context(&filtered);
        
        // 5. Generate answer
        let prompt = self.build_prompt(question, &context);
        let response = self.llm.generate(GenerateRequest {
            model: self.config.model.clone(),
            prompt,
            ..Default::default()
        }).await?;
        
        Ok(RAGResponse {
            answer: response.response,
            sources: filtered.iter().map(|r| r.id.clone()).collect(),
            confidence: self.calculate_confidence(&filtered),
        })
    }
    
    fn build_context(&self, results: &[SearchResult]) -> String {
        results.iter()
            .take(self.config.context_chunks)
            .map(|r| format!("Source: {}\n{}\n", r.id, r.metadata.get("content").unwrap_or(&String::new())))
            .collect()
    }
}
```

---

## Performance Optimization

### Batching

```rust
/// Batch embedding generation for efficiency
pub async fn embed_batch(texts: &[String], batch_size: usize) -> Result<Vec<Vec<f32>>> {
    let mut all_embeddings = Vec::with_capacity(texts.len());
    
    for chunk in texts.chunks(batch_size) {
        let embeddings = ollama.embed(EmbedRequest {
            model: "nomic-embed-text".to_string(),
            prompt: chunk.to_vec(),
        }).await?;
        
        all_embeddings.extend(embeddings);
    }
    
    Ok(all_embeddings)
}
```

### Caching

```rust
/// LRU cache for embeddings
pub struct EmbeddingCache {
    cache: RwLock<LruCache<String, Vec<f32>>>,
}

impl EmbeddingCache {
    pub async fn get_or_compute(&self, text: &str, embedder: &dyn Embedder) -> Result<Vec<f32>> {
        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(embedding) = cache.get(&text.to_string()) {
                return Ok(embedding.clone());
            }
        }
        
        // Compute and cache
        let embedding = embedder.embed(text).await?;
        {
            let mut cache = self.cache.write().await;
            cache.put(text.to_string(), embedding.clone());
        }
        
        Ok(embedding)
    }
}
```

### Streaming

```rust
/// Stream LLM responses for better UX
pub async fn generate_streaming(
    request: GenerateRequest,
    callback: impl Fn(String),
) -> Result<String> {
    let mut full_response = String::new();
    
    let response = client.post(&url)
        .json(&GenerateRequest { stream: true, ..request })
        .send()
        .await?;
    
    let mut stream = response.bytes_stream();
    
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text: StreamResponse = serde_json::from_slice(&chunk)?;
        
        callback(text.response.clone());
        full_response.push_str(&text.response);
        
        if text.done {
            break;
        }
    }
    
    Ok(full_response)
}
```

---

## Offline Operation

### Model Preloading

```rust
/// Preload models for offline operation
pub async fn preload_models(models: &[String]) -> Result<()> {
    for model in models {
        // Check if model is downloaded
        if !ollama.check_model(model).await? {
            println!("Downloading model: {}", model);
            ollama.pull_model(model).await?;
        }
        
        // Warm up the model
        ollama.generate(GenerateRequest {
            model: model.clone(),
            prompt: "Hello".to_string(),
            ..Default::default()
        }).await?;
    }
    
    Ok(())
}
```

### Fallback Strategy

```rust
/// Fallback to smaller models when resources are constrained
pub fn get_fallback_model(primary: &str) -> &str {
    match primary {
        "llama3.1:70b" => "llama3.1:8b",
        "llama3.1:8b" => "llama3.2:3b",
        "llama3.2:3b" => "phi3:mini",
        "mixtral:8x7b" => "mistral:7b",
        _ => "phi3:mini",
    }
}
```
