use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct VisionRequest {
    model: String,
    messages: Vec<VisionMessage>,
    max_tokens: u32,
    temperature: f64,
}

#[derive(Serialize)]
struct VisionMessage {
    role: String,
    content: Vec<VisionContent>,
}

#[derive(Serialize)]
struct VisionContent {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_url: Option<ImageUrl>,
}

#[derive(Serialize)]
struct ImageUrl {
    url: String,
}

#[derive(Deserialize)]
struct VisionResponse {
    choices: Vec<VisionChoice>,
}

#[derive(Deserialize)]
struct VisionChoice {
    message: VisionChoiceMessage,
}

#[derive(Deserialize)]
struct VisionChoiceMessage {
    content: String,
}

pub async fn ocr_image(image_base64: &str, api_key: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Create HTTP client failed: {}", e))?;

    let body = VisionRequest {
        model: "Qwen/Qwen3-VL-30B-A3B-Instruct".to_string(),
        messages: vec![VisionMessage {
            role: "user".to_string(),
            content: vec![
                VisionContent {
                    content_type: "image_url".to_string(),
                    text: None,
                    image_url: Some(ImageUrl {
                        url: format!("data:image/png;base64,{}", image_base64),
                    }),
                },
                VisionContent {
                    content_type: "text".to_string(),
                    text: Some(
                        "Output all code from the image line by line, include all indentation and special characters. Do not miss any line. Output code only, no extra text.".to_string(),
                    ),
                    image_url: None,
                },
            ],
        }],
        max_tokens: 1024,
        temperature: 0.1,
    };

    let resp = client
        .post("https://api.siliconflow.cn/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("SiliconFlow request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("SiliconFlow HTTP {}: {}", status, text));
    }

    let data: VisionResponse = resp
        .json()
        .await
        .map_err(|e| format!("Parse SiliconFlow response failed: {}", e))?;

    Ok(data.choices[0].message.content.clone())
}

pub async fn solve_problem(code: &str, api_key: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Create HTTP client failed: {}", e))?;

    let prompt = format!(
        "You are an interview coding assistant. The user has the following code from a coding problem:\n\
         ```\n{}\n```\n\n\
         Please do three things:\n\
         1. Explain the problem and the solution approach\n\
         2. Provide the optimized complete code\n\
         3. Analyze time and space complexity\n\
         Answer in Chinese.",
        code
    );

    let body = serde_json::json!({
        "model": "deepseek-v4-flash",
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ],
        "max_tokens": 2048,
        "temperature": 0.3
    });

    let resp = client
        .post("https://api.deepseek.com/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("DeepSeek request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("DeepSeek HTTP {}: {}", status, text));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse DeepSeek response failed: {}", e))?;

    Ok(data["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

pub async fn analyze_screenshot(
    image_base64: &str,
    siliconflow_key: &str,
    deepseek_key: &str,
) -> Result<(String, String), String> {
    let code = ocr_image(image_base64, siliconflow_key).await?;
    let solution = solve_problem(&code, deepseek_key).await?;
    Ok((code, solution))
}
