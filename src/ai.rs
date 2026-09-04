use moka::future::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info};

use crate::source::TrackMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiProvider {
    Gemini,
    Claude,
    OpenAi,
    OpenAiCompatible,
}

impl AiProvider {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "claude" | "anthropic" => Self::Claude,
            "openai" => Self::OpenAi,
            "openai_compatible" | "openai-compatible" | "grok" | "qwen" | "groq" | "ollama"
            | "deepseek" | "openrouter" => Self::OpenAiCompatible,
            _ => Self::Gemini,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoodCuration {
    pub commentary: String,
    pub query_seeds: Vec<String>,
}

pub struct AiClient {
    http_client: reqwest::Client,
    provider: AiProvider,
    api_key: String,
    model: String,
    base_url: Option<String>,
    web_search_enabled: bool,
    is_local: bool,
    trivia_cache: Cache<String, String>,
}

impl AiClient {
    pub fn init() -> Self {
        let provider_str = std::env::var("LLM_PROVIDER").unwrap_or_else(|_| "gemini".to_string());
        let provider = AiProvider::from_str(&provider_str);

        // Resolve API key. Order of precedence:
        //   1. LLM_API_KEY (generic, always wins)
        //   2. Provider-specific key matching the selected provider
        //      (GEMINI_API_KEY for gemini, CLAUDE_API_KEY for claude, etc)
        //   3. Ollama is a special case: no API key needed, but the local
        //      server should be reachable.
        let api_key = std::env::var("LLM_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                // Provider-specific fallback. Pick the one matching the
                // selected provider so users with multiple keys set don't
                // accidentally route Claude's key to Gemini, etc.
                let env_var = match provider {
                    AiProvider::Gemini => "GEMINI_API_KEY",
                    AiProvider::Claude => "CLAUDE_API_KEY",
                    AiProvider::OpenAi | AiProvider::OpenAiCompatible => "OPENAI_API_KEY",
                };
                std::env::var(env_var).ok().filter(|s| !s.trim().is_empty())
            })
            .or_else(|| {
                // Ollama doesn't need a real API key — but only the
                // OpenAI-compatible path handles it. If the user picked
                // ollama and set no key, use a placeholder so is_enabled()
                // returns true and the request still goes out.
                if matches!(provider, AiProvider::OpenAiCompatible)
                    && provider_str.to_lowercase().trim() == "ollama"
                {
                    Some("ollama".to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default()
            .trim()
            .to_string();

        // Default model selection. For openai_compatible, the model depends on which
        // provider was selected (grok vs groq vs ollama etc) since they all use
        // different model namespaces. Falls back to a sensible default per provider.
        let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| {
            match provider {
                AiProvider::Gemini => "gemini-1.5-flash".to_string(),
                AiProvider::Claude => "claude-3-5-haiku-latest".to_string(),
                AiProvider::OpenAi => "gpt-4o-mini".to_string(),
                AiProvider::OpenAiCompatible => {
                    match provider_str.to_lowercase().trim() {
                        "grok" => "grok-2-latest".to_string(),
                        "groq" => "llama-3.1-70b-versatile".to_string(),
                        "qwen" => "qwen-plus".to_string(),
                        "ollama" => "llama3.1".to_string(),
                        "deepseek" => "deepseek-chat".to_string(),
                        "openrouter" => "anthropic/claude-3.5-sonnet".to_string(),
                        _ => "gpt-4o-mini".to_string(), // generic openai_compatible
                    }
                }
            }
        });

        let base_url = std::env::var("LLM_BASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let web_search_enabled = std::env::var("LLM_WEB_SEARCH")
            .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true);

        if !api_key.is_empty() {
            info!(
                "AI DJ Client initialized: Provider={:?}, Model={}, WebSearch={}, BaseUrl={:?}",
                provider, model, web_search_enabled, base_url
            );
        } else {
            info!("AI DJ Client: No LLM_API_KEY provided. AI features disabled (commands will skip LLM calls and use local fallbacks).");
        }

        Self {
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .connect_timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            provider,
            api_key,
            model,
            base_url,
            web_search_enabled,
            is_local: provider_str.to_lowercase().trim() == "ollama",
            trivia_cache: Cache::builder()
                .max_capacity(200)
                .time_to_live(Duration::from_secs(24 * 3600))
                .build(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.api_key.is_empty()
    }

    /// Returns true only if the client is fully usable — has a key AND (for
    /// openai_compatible providers) a base URL configured. Without this check,
    /// Ollama with no LLM_BASE_URL would silently send requests to api.openai.com
    /// and get 401. Use this at call sites that need the LLM to actually work.
    pub fn is_usable(&self) -> bool {
        if !self.is_enabled() {
            return false;
        }
        match self.provider {
            AiProvider::Gemini | AiProvider::Claude | AiProvider::OpenAi => true,
            AiProvider::OpenAiCompatible => self.base_url.is_some(),
        }
    }

    /// Returns a short status string for the /ping command's Configuration field.
    /// Format: "Provider=X, Model=Y, Usable=true/false (reason)"
    pub fn diagnostics(&self) -> String {
        if !self.is_enabled() {
            return "Disabled (no API key)".to_string();
        }
        let provider_name = match self.provider {
            AiProvider::Gemini => "Gemini",
            AiProvider::Claude => "Claude",
            AiProvider::OpenAi => "OpenAI",
            AiProvider::OpenAiCompatible => "OpenAI-Compatible",
        };
        let usable = if self.is_usable() {
            "OK".to_string()
        } else {
            "misconfigured (missing LLM_BASE_URL)".to_string()
        };
        format!("{} | {} | {}", provider_name, self.model, usable)
    }

    /// Generic text generation across all supported LLM providers
    pub async fn generate_text(&self, system: &str, prompt: &str) -> Result<String, String> {
        if !self.is_enabled() {
            return Err("LLM_API_KEY not configured".to_string());
        }
        if !self.is_usable() {
            return Err("LLM is enabled but not properly configured (missing LLM_BASE_URL for openai_compatible provider)".to_string());
        }

        match self.provider {
            AiProvider::Gemini => self.call_gemini(system, prompt).await,
            AiProvider::Claude => {
                let enriched_prompt = self.enrich_with_web_search(prompt).await;
                self.call_claude(system, &enriched_prompt).await
            }
            AiProvider::OpenAi | AiProvider::OpenAiCompatible => {
                let enriched_prompt = self.enrich_with_web_search(prompt).await;
                self.call_openai_compatible(system, &enriched_prompt).await
            }
        }
    }

    async fn enrich_with_web_search(&self, prompt: &str) -> String {
        // Local providers (Ollama) can't use real-time web search context
        // effectively, and the DuckDuckGo fallback adds ~1-2s of latency
        // for no benefit. Skip for local providers unless explicitly enabled.
        if !self.web_search_enabled || self.is_local {
            return prompt.to_string();
        }

        if let Some(snippets) = self.search_web_duckduckgo(prompt).await {
            format!(
                "{}\n\n[Real-time Web Search Context]:\n{}\n(Use the real-time search context above to ensure accuracy)",
                prompt, snippets
            )
        } else {
            prompt.to_string()
        }
    }

    /// Free, zero-token real-time web search fallback for non-Gemini providers (Claude, OpenAI, Grok, Qwen, etc.)
    pub async fn search_web_duckduckgo(&self, query: &str) -> Option<String> {
        let clean_q = query
            .lines()
            .next()
            .unwrap_or(query)
            .trim_start_matches("Song:")
            .trim_start_matches("User requested vibe/mood:")
            .trim_start_matches("User requested mood:")
            .trim_matches('"')
            .trim();

        if clean_q.is_empty() {
            return None;
        }

        let res = self
            .http_client
            .get("https://html.duckduckgo.com/html/")
            .query(&[("q", clean_q)])
            .header(
                reqwest::header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .send()
            .await
            .ok()?;

        if !res.status().is_success() {
            return None;
        }

        let html = res.text().await.ok()?;
        let mut snippets = Vec::new();

        for part in html.split("class=\"result__snippet\"") {
            if let Some(start_tag) = part.find('>') {
                let after_tag = &part[start_tag + 1..];
                if let Some(end_tag) = after_tag.find("</a>") {
                    let raw_snippet = &after_tag[..end_tag];
                    let clean = Self::strip_html_tags(raw_snippet);
                    if !clean.trim().is_empty() && clean.len() > 15 {
                        snippets.push(format!("- {}", clean.trim()));
                        if snippets.len() >= 3 {
                            break;
                        }
                    }
                }
            }
        }

        if snippets.is_empty() {
            None
        } else {
            Some(snippets.join("\n"))
        }
    }

    fn strip_html_tags(input: &str) -> String {
        let mut output = String::with_capacity(input.len());
        let mut in_tag = false;
        for c in input.chars() {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            } else if !in_tag {
                output.push(c);
            }
        }
        output
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
    }

    async fn call_gemini(&self, system: &str, prompt: &str) -> Result<String, String> {
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        let mut body = serde_json::json!({
            "system_instruction": {
                "parts": [{ "text": system }]
            },
            "contents": [{
                "role": "user",
                "parts": [{ "text": prompt }]
            }],
            "generationConfig": {
                "temperature": 0.7,
                "maxOutputTokens": 800
            }
        });

        // Add Google Search grounding for Gemini if enabled
        if self.web_search_enabled {
            body["tools"] = serde_json::json!([
                { "google_search": {} }
            ]);
        }

        let res = self
            .http_client
            .post(&endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Gemini HTTP request error: {}", e))?;

        // Graceful fallback retry without tools if Gemini returns error on tools
        let res = if !res.status().is_success() && self.web_search_enabled {
            body.as_object_mut().map(|m| m.remove("tools"));
            self.http_client.post(&endpoint).json(&body).send().await.unwrap_or(res)
        } else {
            res
        };

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            error!("Gemini API error {}: {}", status, err_body);
            return Err(format!("Gemini API error: {}", status));
        }

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse Gemini JSON: {}", e))?;

        let text = json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if text.is_empty() {
            Err("Empty response from Gemini".to_string())
        } else {
            Ok(text)
        }
    }

    async fn call_claude(&self, system: &str, prompt: &str) -> Result<String, String> {
        let endpoint = self
            .base_url
            .as_deref()
            .unwrap_or("https://api.anthropic.com/v1/messages");

        let body = serde_json::json!({
            "model": self.model,
            "system": system,
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "max_tokens": 800
        });

        let res = self
            .http_client
            .post(endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Claude HTTP request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            error!("Claude API error {}: {}", status, err_body);
            return Err(format!("Claude API error: {}", status));
        }

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse Claude JSON: {}", e))?;

        let text = json
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if text.is_empty() {
            Err("Empty response from Claude".to_string())
        } else {
            Ok(text)
        }
    }

    async fn call_openai_compatible(&self, system: &str, prompt: &str) -> Result<String, String> {
        let default_url = "https://api.openai.com/v1/chat/completions".to_string();
        let base = self.base_url.as_ref().unwrap_or(&default_url);
        let endpoint = if base.ends_with("/chat/completions") {
            base.clone()
        } else {
            format!("{}/chat/completions", base.trim_end_matches('/'))
        };

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": prompt }
            ],
            "temperature": 0.7,
            "max_tokens": 800
        });

        let res = self
            .http_client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("OpenAI-compatible HTTP request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            error!("OpenAI-compatible API error {}: {}", status, err_body);
            return Err(format!("OpenAI-compatible API error: {}", status));
        }

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse OpenAI-compatible JSON: {}", e))?;

        let text = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if text.is_empty() {
            Err("Empty response from OpenAI-compatible API".to_string())
        } else {
            Ok(text)
        }
    }

    /// Generates a charismatic radio DJ review of the server's music taste
    pub async fn review_taste(&self, history: &[TrackMetadata]) -> Result<String, String> {
        let is_id = crate::lang::is_id();
        let song_list: Vec<String> = history
            .iter()
            .rev()
            .take(15)
            .map(|t| format!("- {} by {}", t.title, t.author.as_deref().unwrap_or("Unknown")))
            .collect();

        if song_list.is_empty() {
            return Ok(if is_id {
                "🎙️ **DJ:** Belum ada riwayat lagu nih! Yuk putar lagu favorit kalian biar DJ tahu selera musik server ini!".to_string()
            } else {
                "🎙️ **DJ:** No playback history yet! Spin some tracks so I can catch the vibe of this server!".to_string()
            });
        }

        let history_str = song_list.join("\n");

        let system = if is_id {
            "Kamu adalah penyiar radio musik (Radio DJ) yang sangat karismatik, santai, gaul, dan penuh semangat di Discord. \
             Tugasmu adalah menganalisis riwayat lagu server dan memberikan ulasan selera musik mereka dalam 2-3 kalimat pendek yang menarik. \
             Gunakan gaya bicara penyiar radio santai khas anak muda Indonesia. Awali dengan '🎙️ **DJ:** '."
        } else {
            "You are a charismatic, witty, and energetic Discord radio DJ. \
             Your job is to review the server's recent playback history and give a fun, 2-3 sentence commentary on their musical taste and current vibe. \
             Start your response with '🎙️ **DJ:** '."
        };

        let prompt = format!(
            "Here is the server's recent playback history:\n{}\n\nGive your charismatic DJ commentary on their current music taste.",
            history_str
        );

        self.generate_text(system, &prompt).await
    }

    /// Curates songs based on an abstract user mood or natural language prompt
    pub async fn curate_mood(&self, mood: &str, history: &[TrackMetadata]) -> Result<MoodCuration, String> {
        let is_id = crate::lang::is_id();
        let recent: Vec<String> = history
            .iter()
            .rev()
            .take(5)
            .map(|t| format!("{} by {}", t.title, t.author.as_deref().unwrap_or("")))
            .collect();

        let system = if is_id {
            "Kamu adalah AI Music Director & Radio DJ profesional. \
             User memberikan deskripsi suasana/mood musik yang mereka inginkan. \
             Tugasmu adalah memberikan respon JSON dengan format: \
             {\n  \"commentary\": \"Kalimat pendek 1-2 kalimat ala penyiar radio yang menyambut mood user\",\n  \"query_seeds\": [\"Title 1 - Artist 1\", \"Title 2 - Artist 2\", \"Title 3 - Artist 3\", \"Title 4 - Artist 4\", \"Title 5 - Artist 5\"]\n}\n \
             PENTING: Hanya keluarkan format JSON murni tanpa markdown kode block (```). \
             Pastikan hanya memilih lagu standar individual dengan durasi normal di bawah 10 menit (JANGAN pilih kompilasi album penuh, mix 1 jam, atau extended loop)."
        } else {
            "You are a professional AI Music Director & Radio DJ. \
             The user provides an abstract mood, vibe, or musical prompt. \
             Your job is to return a pure JSON object in this exact schema: \
             {\n  \"commentary\": \"A 1-2 sentence charismatic DJ line introducing the vibe\",\n  \"query_seeds\": [\"Title 1 - Artist 1\", \"Title 2 - Artist 2\", \"Title 3 - Artist 3\", \"Title 4 - Artist 4\", \"Title 5 - Artist 5\"]\n}\n \
             IMPORTANT: Output pure valid JSON without markdown code fences. \
             Ensure you only select individual standalone songs under 10 minutes in duration (DO NOT pick full album compilations, 1-hour mixes, or extended loops)."
        };

        let prompt = format!(
            "User requested mood: \"{}\"\nRecent server history for flavor reference:\n{}\n\nPick 5 real, popular, fitting standalone songs matching this mood (duration strictly under 10 minutes, no full albums or long mixes).",
            mood,
            recent.join("\n")
        );

        let raw = self.generate_text(system, &prompt).await?;
        let clean = raw
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        if let Ok(curation) = serde_json::from_str::<MoodCuration>(clean) {
            if !curation.query_seeds.is_empty() {
                return Ok(curation);
            }
        }

        // Fallback parsing if LLM returned text lines
        let lines: Vec<String> = raw
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-').trim().to_string())
            .filter(|l| !l.is_empty())
            .take(5)
            .collect();

        Ok(MoodCuration {
            commentary: format!("🎙️ **DJ:** Meracik lagu spesial untuk mood: \"{}\"!", mood),
            query_seeds: if lines.is_empty() { vec![mood.to_string()] } else { lines },
        })
    }

    /// Generates a quick charismatic DJ intro welcoming the user's mood without generating fake song seeds
    pub async fn comment_mood(&self, mood: &str) -> Result<String, String> {
        let is_id = crate::lang::is_id();
        let system = if is_id {
            "Kamu adalah penyiar radio musik (Radio DJ) di Discord yang gaul, santai, dan karismatik. \
             User sedang mencari rekomendasi lagu dengan mood atau kata kunci tertentu. \
             Berikan 1-2 kalimat sapaan DJ yang seru dan bersemangat menyambut pilihan mood user tersebut. \
             Awali dengan '🎙️ **DJ:** '."
        } else {
            "You are a charismatic, energetic Discord Radio DJ. \
             The user is searching for music recommendations for a specific mood or prompt. \
             Give a fun, 1-2 sentence DJ intro welcoming this vibe. \
             Start with '🎙️ **DJ:** '."
        };

        let prompt = format!("User requested vibe/mood: \"{}\"", mood);
        self.generate_text(system, &prompt).await
    }

    /// Fetches an engaging 1-sentence trivia fact about a song
    pub async fn get_trivia(&self, title: &str, author: &str) -> Result<String, String> {
        let cache_key = format!("{}::{}", title.to_lowercase(), author.to_lowercase());
        if let Some(cached) = self.trivia_cache.get(&cache_key).await {
            return Ok(cached);
        }

        let is_id = crate::lang::is_id();
        let system = if is_id {
            "Kamu adalah ensiklopedia musik cerdas. Berikan TEPAT 1 kalimat fakta unik / trivia menarik tentang lagu dan artis berikut. \
             Awali dengan '💡 **Trivia:** '. Jawab singkat, padat, dan menarik dalam bahasa Indonesia."
        } else {
            "You are a music trivia expert. Give EXACTLY 1 interesting fun fact about the given song and artist. \
             Start with '💡 **Trivia:** '. Keep it concise, engaging, and under 25 words."
        };

        let prompt = format!("Song: \"{}\" by \"{}\"", title, author);
        let trivia = self.generate_text(system, &prompt).await?;
        self.trivia_cache.insert(cache_key, trivia.clone()).await;
        Ok(trivia)
    }
}
