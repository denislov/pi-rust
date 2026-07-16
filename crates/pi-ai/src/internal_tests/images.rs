use pi_ai::images::{
    AssistantImages, ImageContent, ImagesContext, ImagesModel, ImagesModelCost, ImagesModelOutput,
    TextContent,
};

fn image_model() -> ImagesModel {
    ImagesModel {
        id: "openrouter/image-test".into(),
        name: "Image Test".into(),
        api: "openrouter-images".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        output: vec![ImagesModelOutput::Image, ImagesModelOutput::Text],
        cost: ImagesModelCost {
            input: 1.0,
            output: 2.0,
            cache_read: 0.1,
            cache_write: 0.5,
        },
        headers: None,
    }
}

#[test]
fn openrouter_image_request_maps_text_image_and_modalities() {
    let model = image_model();
    let ctx = ImagesContext {
        input: vec![
            pi_ai::images::ImageInput::Text(TextContent {
                text: "draw".into(),
            }),
            pi_ai::images::ImageInput::Image(ImageContent {
                data: "abc".into(),
                mime_type: "image/png".into(),
            }),
        ],
    };

    let request = pi_ai::images::openrouter::build_request(&model, &ctx);
    let json = serde_json::to_value(request).unwrap();

    assert_eq!(json["model"], "openrouter/image-test");
    assert_eq!(json["stream"], false);
    assert_eq!(json["modalities"], serde_json::json!(["image", "text"]));
    assert_eq!(json["messages"][0]["content"][0]["type"], "text");
    assert_eq!(
        json["messages"][0]["content"][1]["image_url"]["url"],
        "data:image/png;base64,abc"
    );
}

#[test]
fn openrouter_image_response_extracts_text_images_and_usage() {
    let model = image_model();
    let response = serde_json::json!({
        "id": "gen_1",
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 4,
            "prompt_tokens_details": {
                "cached_tokens": 3,
                "cache_write_tokens": 1
            }
        },
        "choices": [{
            "message": {
                "content": "caption",
                "images": [{"image_url": {"url": "data:image/png;base64,abc"}}]
            }
        }]
    });

    let output: AssistantImages =
        pi_ai::images::openrouter::response_to_images(&model, response).unwrap();

    assert_eq!(output.response_id.as_deref(), Some("gen_1"));
    assert!(matches!(
        output.output[0],
        pi_ai::images::ImageOutput::Text(_)
    ));
    assert!(matches!(
        output.output[1],
        pi_ai::images::ImageOutput::Image(_)
    ));
    assert_eq!(output.usage.unwrap().cache_read, 2);
    assert_eq!(output.stop_reason, "stop");
}
// Internal image-provider tests.
