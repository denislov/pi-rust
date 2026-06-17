use pi_coding_agent::interactive::UiEvent;
use pi_coding_agent::interactive::{Transcript, TranscriptItem};

#[test]
fn transcript_scrolls_within_bounds() {
    let mut transcript = Transcript::new();
    for i in 0..20 {
        transcript.push(TranscriptItem::user(format!("message {i}")));
    }
    transcript.scroll_page_up(5);
    assert_eq!(transcript.scroll_offset(), 5);
    transcript.scroll_page_down(2);
    assert_eq!(transcript.scroll_offset(), 3);
    transcript.scroll_to_bottom();
    assert_eq!(transcript.scroll_offset(), 0);
}

#[test]
fn transcript_keeps_scrolled_view_locked_when_new_output_arrives() {
    let mut transcript = Transcript::new();
    for i in 0..20 {
        transcript.push(TranscriptItem::user(format!("message {i}")));
    }
    transcript.scroll_page_up(5);

    transcript.apply_event(UiEvent::AssistantDelta {
        text: "new output".to_string(),
    });

    assert!(transcript.scroll_offset() > 5);
    assert!(transcript.has_new_output_below());
    transcript.scroll_page_down(usize::MAX);
    assert_eq!(transcript.scroll_offset(), 0);
    assert!(!transcript.has_new_output_below());
}

#[test]
fn system_item_is_pushed_and_rendered_as_a_line() {
    let mut transcript = Transcript::new();
    transcript.push(TranscriptItem::system("welcome to pi"));
    assert_eq!(transcript.items().len(), 1);
    match transcript.items()[0] {
        TranscriptItem::System { ref text } => assert_eq!(text, "welcome to pi"),
        _ => panic!("expected System item"),
    }
}

#[test]
fn system_item_scrolls_like_other_items() {
    let mut transcript = Transcript::new();
    transcript.push(TranscriptItem::system("welcome"));
    transcript.scroll_page_up(2);
    assert_eq!(transcript.scroll_offset(), 2);
    transcript.scroll_to_bottom();
    assert_eq!(transcript.scroll_offset(), 0);
}
