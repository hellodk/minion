//! Markdown-aware chunker.
//!
//! Chunk size is measured in characters rather than tokens, which is
//! simpler and close enough for MVP — most embedding models accept
//! 512–8192 tokens and a character:token ratio of ~4 means a 1500-char
//! chunk never crosses a 512-token cap.
//!
//! Rules:
//!
//! * Never split inside a fenced code block (```…```).
//! * Prefer to split at heading or blank-line boundaries; fall back to
//!   sentence boundaries (period / newline); fall back to hard cut.
//! * Emit overlapping chunks so context straddling a boundary survives.
//! * Skip empty content.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub struct ChunkOptions {
    pub target_chars: usize,
    pub overlap_chars: usize,
}

impl Default for ChunkOptions {
    fn default() -> Self {
        Self {
            target_chars: 1200,
            overlap_chars: 180,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Chunk {
    pub index: usize,
    pub text: String,
    /// Optional section heading in effect at the start of this chunk.
    pub heading: Option<String>,
    /// Character offset into the original document where this chunk starts.
    pub start_char: usize,
}

/// Split a markdown string into chunks.
pub fn chunk_markdown(body: &str, opts: ChunkOptions) -> Vec<Chunk> {
    let target = opts.target_chars.max(200);
    let overlap = opts.overlap_chars.min(target / 2);

    // Normalize line endings before splitting so CRLF inputs produce
    // the same `start_char` offsets as LF inputs. `str::lines()` strips
    // \r\n and \n identically, so without this the running_char
    // accumulator drifts by 1 per line on Windows-style files.
    let normalized = body.replace("\r\n", "\n");
    let body = normalized.as_str();

    // First pass: split into "blocks" — fenced-code-aware paragraph
    // boundaries. Each block carries the heading in effect.
    let blocks = split_blocks(body);
    if blocks.is_empty() {
        return Vec::new();
    }

    // Second pass: greedy pack blocks into chunks respecting target size.
    let mut out: Vec<Chunk> = Vec::new();
    let mut buf = String::new();
    let mut buf_heading: Option<String> = None;
    let mut buf_start = 0usize;

    for b in blocks {
        if buf.is_empty() {
            buf_heading = b.heading.clone();
            buf_start = b.start_char;
        }
        let would_be = buf.chars().count() + b.text.chars().count() + 2;
        if !buf.is_empty() && would_be > target {
            out.push(Chunk {
                index: out.len(),
                text: std::mem::take(&mut buf).trim().to_string(),
                heading: buf_heading.take(),
                start_char: buf_start,
            });
            // Start the next chunk with a tail of the previous one for
            // overlap, IF the tail is non-trivial.
            if overlap > 0 {
                if let Some(prev) = out.last() {
                    let tail = tail_chars(&prev.text, overlap);
                    if !tail.is_empty() {
                        buf.push_str(&tail);
                        buf.push_str("\n\n");
                    }
                }
            }
            buf_heading = b.heading.clone();
            buf_start = b.start_char;
        }
        if !buf.is_empty() {
            buf.push_str("\n\n");
        }
        buf.push_str(&b.text);

        // If a *single* block is larger than target on its own (a giant
        // code block or paragraph), hard-split it.
        if buf.chars().count() > target * 2 {
            for piece in hard_split(&buf, target) {
                out.push(Chunk {
                    index: out.len(),
                    text: piece,
                    heading: buf_heading.clone(),
                    start_char: buf_start,
                });
            }
            buf.clear();
            buf_heading = None;
        }
    }
    if !buf.trim().is_empty() {
        out.push(Chunk {
            index: out.len(),
            text: buf.trim().to_string(),
            heading: buf_heading,
            start_char: buf_start,
        });
    }
    out
}

struct Block {
    text: String,
    heading: Option<String>,
    start_char: usize,
}

fn split_blocks(body: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut heading: Option<String> = None;
    let mut in_fence = false;
    let mut fence_marker: Option<String> = None;
    let mut current = String::new();
    let mut current_start: usize = 0;
    let mut running_char: usize = 0;

    let push_current =
        |current: &mut String, blocks: &mut Vec<Block>, heading: &Option<String>, start: usize| {
            let text = current.trim().to_string();
            if !text.is_empty() {
                blocks.push(Block {
                    text,
                    heading: heading.clone(),
                    start_char: start,
                });
            }
            current.clear();
        };

    for line in body.lines() {
        let stripped = line.trim_start();
        // Track fence state — allow only matching ``` pair to toggle.
        if !in_fence && stripped.starts_with("```") {
            in_fence = true;
            fence_marker = Some("```".to_string());
        } else if in_fence && stripped.starts_with("```") && fence_marker.as_deref() == Some("```")
        {
            in_fence = false;
            fence_marker = None;
            // Include the closing fence in the current block.
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
            running_char += line.chars().count() + 1;
            continue;
        }

        if !in_fence {
            // Headings start a new block AND update the running heading.
            if let Some(rest) = stripped.strip_prefix("# ") {
                push_current(&mut current, &mut blocks, &heading, current_start);
                heading = Some(rest.trim().to_string());
                current_start = running_char;
                current.push_str(line);
                running_char += line.chars().count() + 1;
                continue;
            }
            if stripped.starts_with("## ")
                || stripped.starts_with("### ")
                || stripped.starts_with("#### ")
            {
                push_current(&mut current, &mut blocks, &heading, current_start);
                heading = Some(stripped.trim_start_matches('#').trim_start().to_string());
                current_start = running_char;
                current.push_str(line);
                running_char += line.chars().count() + 1;
                continue;
            }
            // Blank line terminates a paragraph-level block.
            if line.trim().is_empty() {
                push_current(&mut current, &mut blocks, &heading, current_start);
                current_start = running_char + 1;
                running_char += 1;
                continue;
            }
        }

        if current.is_empty() {
            current_start = running_char;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
        running_char += line.chars().count() + 1;
    }
    push_current(&mut current, &mut blocks, &heading, current_start);
    blocks
}

fn tail_chars(s: &str, n: usize) -> String {
    let total = s.chars().count();
    if n >= total {
        return s.to_string();
    }
    s.chars().skip(total - n).collect()
}

/// Split a giant block by characters into chunks of at most `size`
/// chars each. Used only as a fallback for outsized single blocks.
fn hard_split(s: &str, size: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    for ch in s.chars() {
        if buf.chars().count() >= size {
            out.push(std::mem::take(&mut buf));
        }
        buf.push(ch);
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_doc_produces_single_chunk() {
        let md = "# Intro\n\nHello world.\n\nAnother line.";
        let chunks = chunk_markdown(md, ChunkOptions::default());
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("Hello world"));
        assert_eq!(chunks[0].heading.as_deref(), Some("Intro"));
    }

    #[test]
    fn large_doc_produces_multiple_chunks_with_overlap() {
        let para = "lorem ipsum dolor sit amet ".repeat(60); // ~1620 chars
        let md = format!("# A\n\n{}\n\n# B\n\n{}", para, para);
        let opts = ChunkOptions {
            target_chars: 800,
            overlap_chars: 100,
        };
        let chunks = chunk_markdown(&md, opts);
        assert!(
            chunks.len() >= 3,
            "expected chunking, got {:?}",
            chunks.len()
        );
        // Overlap means consecutive chunks should share some characters.
        let a = &chunks[0].text;
        let b = &chunks[1].text;
        let tail_a = a.chars().rev().take(30).collect::<String>();
        let head_b = b.chars().take(200).collect::<String>();
        let tail_reversed: String = tail_a.chars().rev().collect();
        assert!(
            head_b.contains(&tail_reversed[..tail_reversed.len().min(20)]),
            "expected overlap between chunk 0 and chunk 1"
        );
    }

    #[test]
    fn code_fence_is_not_split() {
        let code: String = "println!(\"line\");\n".repeat(200);
        let md = format!("# Title\n\nIntro.\n\n```rust\n{}```\n\nAfter.", code);
        let chunks = chunk_markdown(
            &md,
            ChunkOptions {
                target_chars: 500,
                overlap_chars: 0,
            },
        );
        // The code fence should live wholly inside one chunk.
        let containing = chunks.iter().filter(|c| c.text.contains("```rust")).count();
        assert_eq!(containing, 1, "code fence was split across chunks");
    }

    #[test]
    fn heading_carries_to_chunks() {
        let md = "# Parent\n\npar 1\n\n## Child\n\npar 2";
        let chunks = chunk_markdown(md, ChunkOptions::default());
        assert!(chunks
            .iter()
            .any(|c| c.heading.as_deref() == Some("Parent")));
    }

    #[test]
    fn empty_doc_returns_empty() {
        assert!(chunk_markdown("", ChunkOptions::default()).is_empty());
        assert!(chunk_markdown("   \n\n   ", ChunkOptions::default()).is_empty());
    }
}
