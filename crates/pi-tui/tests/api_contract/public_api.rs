//! Stable TUI facade import coverage.

use pi_tui::api::component::{
    BackgroundFn as ApiBackgroundFn, Box as ApiBox, CURSOR_MARKER as API_CURSOR_MARKER,
    Component as ApiComponent, CursorPosition as ApiCursorPosition,
    DefaultTextStyle as ApiDefaultTextStyle, Editor as ApiEditor,
    LoaderIndicatorOptions as ApiLoaderIndicatorOptions, Markdown as ApiMarkdown,
    OverlayMargin as ApiOverlayMargin, OverlayOptions as ApiOverlayOptions,
    OverlayVisibleFn as ApiOverlayVisibleFn, SettingsSubmenuDone as ApiSettingsSubmenuDone,
    SizeValue as ApiSizeValue, Text as ApiText, extract_cursor_marker as api_extract_cursor_marker,
};
use pi_tui::api::component::{
    Box as TuiBox, CancellableLoader, Container, Image, Loader, Markdown, SelectItem,
    SelectorDialog, SelectorDialogOptions, SettingItem, SettingsList, SettingsListOptions, Spacer,
    Text, TruncatedText,
};
use pi_tui::api::input::{
    AutocompleteItem, AutocompleteOptions, KeybindingsManager, SlashCommand, TUI_KEYBINDINGS,
};
use pi_tui::api::input::{
    AutocompleteItem as ApiAutocompleteItem, FuzzyMatch as ApiFuzzyMatch,
    GENERIC_TUI_KEYBINDINGS as API_GENERIC_TUI_KEYBINDINGS, InputEvent as ApiInputEvent,
    Key as ApiKey, KeyEvent as ApiKeyEvent, KeyEventKind as ApiKeyEventKind,
    KeyModifiers as ApiKeyModifiers, KeybindingsManager as ApiKeybindingsManager,
    KillRing as ApiKillRing, MouseButton as ApiMouseButton, MouseEvent as ApiMouseEvent,
    MouseEventKind as ApiMouseEventKind, MouseModifiers as ApiMouseModifiers,
    UndoStack as ApiUndoStack, find_word_backward as api_find_word_backward,
    find_word_forward as api_find_word_forward, fuzzy_filter_indices as api_fuzzy_filter_indices,
    fuzzy_match as api_fuzzy_match, is_key_release as api_is_key_release,
    matches_key as api_matches_key, parse_key as api_parse_key,
    parse_sgr_mouse as api_parse_sgr_mouse,
};
use pi_tui::api::render::{
    Axis as ApiAxis, Color as ApiColor, ColorLevel as ApiColorLevel, Constraint as ApiConstraint,
    FocusRing as ApiFocusRing, Frame as ApiFrame, HitMap as ApiHitMap, HitRegion as ApiHitRegion,
    Layout as ApiLayout, Point as ApiPoint, Rect as ApiRect, RenderScheduler as ApiRenderScheduler,
    RenderStrategy as ApiRenderStrategy, Style as ApiStyle, Tui as ApiTui,
    detect_color_level_from_env as api_detect_color_level_from_env,
    paint_with_level as api_paint_with_level,
    truncate_to_width_with_ellipsis as api_truncate_to_width_with_ellipsis,
    visible_width as api_visible_width, wrap_text_with_ansi as api_wrap_text_with_ansi,
};
use pi_tui::api::render::{
    Color, ColorLevel, Style, Tui, color_level, detect_color_level_from_env, paint_with_level,
    truncate_to_width_with_ellipsis, visible_width, wrap_text_with_ansi,
};
use pi_tui::api::terminal::{
    CellDimensions as ApiCellDimensions, ImageDimensions as ApiImageDimensions,
    ImageProtocol as ApiImageProtocol, ImageRenderOptions as ApiImageRenderOptions,
    NegotiationResult as ApiNegotiationResult, ProcessTerminal as ApiProcessTerminal,
    RgbColor as ApiRgbColor, Terminal as ApiTerminal,
    TerminalCapabilities as ApiTerminalCapabilities, TerminalColorScheme as ApiTerminalColorScheme,
    TerminalMode as ApiTerminalMode, calculate_image_cell_size as api_calculate_image_cell_size,
    delete_all_kitty_images as api_delete_all_kitty_images,
    delete_kitty_image as api_delete_kitty_image,
    is_apple_terminal_session as api_is_apple_terminal_session,
    is_color_scheme_report as api_is_color_scheme_report,
    is_osc11_background_color_response as api_is_osc11_background_color_response,
    normalize_apple_terminal_input as api_normalize_apple_terminal_input,
    parse_color_scheme_report as api_parse_color_scheme_report,
    parse_osc11_background_color as api_parse_osc11_background_color,
};
use pi_tui::api::terminal::{
    CellDimensions, ImageDimensions, ImageProtocol, ImageRenderOptions, ProcessTerminal,
    TerminalCapabilities, TerminalSize, calculate_image_cell_size, delete_all_kitty_images,
    delete_kitty_image, detect_terminal_capabilities_from_env, encode_iterm2, encode_kitty,
    image_dimensions_from_bytes, is_image_line,
};
use pi_tui::api::testing::VirtualTerminal as ApiVirtualTerminal;
use pi_tui::api::testing::VirtualTerminal;
use pi_tui::api::theme::{
    EditorTheme as ApiEditorTheme, ImageTheme as ApiImageTheme, MarkdownTheme as ApiMarkdownTheme,
    SelectListTheme as ApiSelectListTheme, SettingsListTheme as ApiSettingsListTheme,
    TuiTheme as ApiTuiTheme,
};
use pi_tui::api::theme::{ThemeMode, TuiTheme, light_theme};

#[test]
fn generic_tui_symbols_are_importable_from_api_facade() {
    #[allow(clippy::too_many_arguments)]
    fn accepts_types(
        _autocomplete: Option<ApiAutocompleteItem>,
        _box_component: Option<ApiBox>,
        _editor: Option<ApiEditor>,
        _input_event: Option<ApiInputEvent>,
        _key: Option<ApiKey>,
        _keybindings: Option<ApiKeybindingsManager>,
        _markdown: Option<ApiMarkdown>,
        _overlay: Option<ApiOverlayOptions>,
        _process_terminal: Option<ApiProcessTerminal>,
        _terminal_mode: Option<ApiTerminalMode>,
        _render_scheduler: Option<ApiRenderScheduler>,
        _render_strategy: Option<ApiRenderStrategy>,
        _text: Option<ApiText>,
        _theme: Option<ApiTuiTheme>,
        _virtual_terminal: Option<ApiVirtualTerminal>,
    ) {
    }

    accepts_types(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    );

    let image_dimensions = ApiImageDimensions {
        width_px: 18,
        height_px: 18,
    };
    let image_cell_size =
        api_calculate_image_cell_size(image_dimensions, 10, None, ApiCellDimensions::default());
    assert_eq!(image_cell_size.columns, 10);
    assert_eq!(image_cell_size.rows, 5);
    let image_options = ApiImageRenderOptions::default();
    assert!(image_options.preserve_aspect_ratio);
    assert_eq!(api_delete_kitty_image(7), "\u{1b}_Ga=d,d=I,i=7,q=2\u{1b}\\");
    assert_eq!(api_delete_all_kitty_images(), "\u{1b}_Ga=d,d=A,q=2\u{1b}\\");
    let loader_options = ApiLoaderIndicatorOptions {
        frames: vec![".".to_string()],
    };
    assert_eq!(loader_options.frames, vec![".".to_string()]);
    let default_text_style = ApiDefaultTextStyle {
        fg: Some(ApiColor::Ansi256(244)),
        bg: None,
        bold: false,
        italic: true,
        strikethrough: false,
        underline: false,
    };
    assert_eq!(default_text_style.fg, Some(ApiColor::Ansi256(244)));
    let _editor_theme = ApiEditorTheme::default();
    let _image_theme = ApiImageTheme::default();
    let _markdown_theme = ApiMarkdownTheme::default();
    let _select_list_theme = ApiSelectListTheme::default();
    let _settings_list_theme = ApiSettingsListTheme::default();
    let image_capabilities = ApiTerminalCapabilities {
        images: Some(ApiImageProtocol::Kitty),
        true_color: true,
        hyperlinks: true,
    };
    assert_eq!(image_capabilities.images, Some(ApiImageProtocol::Kitty));
    assert_eq!(ApiTerminalMode::default(), ApiTerminalMode::Inline);
    let bounds = ApiRect::new(0, 0, 10, 4);
    assert_eq!(
        ApiLayout::split(bounds, ApiAxis::Vertical, &[ApiConstraint::Fill(1)]),
        [bounds]
    );
    assert_eq!(ApiFrame::new(10, 4).bounds(), bounds);
    assert_eq!(ApiFocusRing::new([1_u8, 2]).current(), Some(1));
    let mouse = api_parse_sgr_mouse("\x1b[<0;3;2M").unwrap();
    assert_eq!(
        mouse,
        ApiMouseEvent {
            kind: ApiMouseEventKind::Down(ApiMouseButton::Left),
            column: 2,
            row: 1,
            modifiers: ApiMouseModifiers::empty(),
        }
    );
    let mut hits = ApiHitMap::new();
    hits.push(ApiHitRegion::new(bounds, "root"));
    assert_eq!(hits.hit(ApiPoint::new(1, 1)), Some(&"root"));

    fn accepts_component<T: ApiComponent>() {}
    fn accepts_terminal<T: ApiTerminal>() {}
    let _ = accepts_component::<ApiText>;
    let _ = accepts_terminal::<ApiVirtualTerminal>;
    let _ = std::any::type_name::<ApiTui<ApiVirtualTerminal>>();

    assert_eq!(api_visible_width("abc"), 3);
    assert_eq!(api_truncate_to_width_with_ellipsis("abcdef", 4), "a...");
    assert_eq!(
        api_wrap_text_with_ansi("hello world", 8),
        vec!["hello", "world"]
    );
    assert_eq!(
        api_detect_color_level_from_env([("TERM", "xterm-256color")]),
        ApiColorLevel::Ansi256
    );
    assert_eq!(
        api_paint_with_level(
            "x",
            &ApiStyle::fg(ApiColor::Ansi256(1)),
            ApiColorLevel::Ansi256,
        ),
        "\u{1b}[38;5;1mx\u{1b}[0m"
    );
}

#[test]
fn component_callback_types_are_importable_from_api_facade() {
    let background: ApiBackgroundFn = Box::new(|line| format!("[{line}]"));
    assert_eq!(background("content"), "[content]");

    let selected = std::rc::Rc::new(std::cell::RefCell::new(None));
    let selected_for_callback = std::rc::Rc::clone(&selected);
    let mut done: ApiSettingsSubmenuDone = Box::new(move |value| {
        *selected_for_callback.borrow_mut() = value;
    });
    done(Some("dark".to_string()));
    drop(done);
    assert_eq!(selected.borrow().as_deref(), Some("dark"));
}

#[test]
fn editor_state_helpers_are_importable_from_api_facade() {
    let mut kill_ring = ApiKillRing::default();
    kill_ring.push("alpha", false, false);
    kill_ring.push("beta", false, false);
    assert_eq!(kill_ring.yank(), Some("beta"));
    assert_eq!(kill_ring.yank_pop(), Some("alpha"));

    let mut undo = ApiUndoStack::new(2);
    undo.push("first".to_string());
    undo.push("second".to_string());
    undo.push("third".to_string());
    assert_eq!(undo.undo("current".to_string()), "third");
    assert_eq!(undo.undo("current".to_string()), "second");
    assert_eq!(undo.undo("current".to_string()), "current");

    let text = "alpha beta";
    assert_eq!(api_find_word_backward(text, text.len()), 6);
    assert_eq!(api_find_word_forward(text, 0), 5);
}

#[test]
fn cursor_helpers_are_importable_from_api_facade() {
    let mut lines = vec!["first".to_string(), format!("ab{}cd", API_CURSOR_MARKER)];
    let position = api_extract_cursor_marker(&mut lines, 2).expect("cursor marker should be found");

    assert_eq!(position, ApiCursorPosition { row: 1, col: 2 });
    assert_eq!(lines[1], "abcd");
}

#[test]
fn fuzzy_helpers_are_importable_from_api_facade() {
    let matched: ApiFuzzyMatch = api_fuzzy_match("df", "Default Profile");
    assert!(matched.matches);

    let items = ["review helper", "default profile", "check helper"];
    assert_eq!(
        api_fuzzy_filter_indices(&items, "helper", |item| *item),
        vec![0, 2]
    );
}

#[test]
fn overlay_layout_types_are_importable_from_api_facade() {
    let mut visible: ApiOverlayVisibleFn = Box::new(|columns, rows| columns > 20 && rows > 5);
    assert!(visible(80, 24));
    assert!(!visible(10, 24));

    let options = ApiOverlayOptions {
        width: Some(ApiSizeValue::Percent(50)),
        max_height: Some(ApiSizeValue::Columns(8)),
        margin: ApiOverlayMargin {
            top: 1,
            right: 2,
            bottom: 3,
            left: 4,
        },
        visible: Some(Box::new(|columns, _rows| columns >= 40)),
        ..ApiOverlayOptions::default()
    };

    assert_eq!(options.width, Some(ApiSizeValue::Percent(50)));
    assert_eq!(options.max_height, Some(ApiSizeValue::Columns(8)));
    assert_eq!(options.margin.left, 4);
    assert!(options.visible.is_some());
}

#[test]
fn input_helpers_are_importable_from_api_facade() {
    let enter = api_parse_key("\r").expect("enter key parses");
    assert_eq!(enter.key, ApiKey::Enter);
    assert_eq!(enter.kind, ApiKeyEventKind::Press);

    assert!(API_GENERIC_TUI_KEYBINDINGS.contains_key("tui.input.submit"));
    assert!(
        !API_GENERIC_TUI_KEYBINDINGS
            .keys()
            .any(|keybinding| keybinding.starts_with("app."))
    );
    let default_keybindings =
        ApiKeybindingsManager::new(API_GENERIC_TUI_KEYBINDINGS.clone(), Default::default());
    assert!(default_keybindings.matches(&ApiInputEvent::Key(enter.clone()), "tui.input.submit"));

    let event = ApiInputEvent::Key(ApiKeyEvent {
        key: ApiKey::Char("x".into()),
        modifiers: ApiKeyModifiers::CTRL,
        kind: ApiKeyEventKind::Press,
    });
    assert!(api_matches_key(&event, "ctrl+x"));
    assert!(!api_is_key_release(&event));

    let release = ApiInputEvent::Key(ApiKeyEvent {
        key: ApiKey::Char("x".into()),
        modifiers: ApiKeyModifiers::CTRL,
        kind: ApiKeyEventKind::Release,
    });
    assert!(api_is_key_release(&release));
    assert!(!api_matches_key(&release, "ctrl+x"));
}

#[test]
fn terminal_negotiation_helpers_are_importable_from_api_facade() {
    let result = ApiNegotiationResult::Done {
        forward: vec!["input".to_string()],
    };
    match result {
        ApiNegotiationResult::Done { forward } => assert_eq!(forward, vec!["input".to_string()]),
        ApiNegotiationResult::Negotiating => panic!("expected done result"),
    }

    let _apple_terminal = api_is_apple_terminal_session();
    assert_eq!(api_normalize_apple_terminal_input("\r", false), "\r");
}

#[test]
fn terminal_color_helpers_are_importable_from_api_facade() {
    assert!(api_is_osc11_background_color_response(
        "\x1b]11;#010203\x07"
    ));
    assert_eq!(
        api_parse_osc11_background_color("\x1b]11;#010203\x07"),
        Some(ApiRgbColor { r: 1, g: 2, b: 3 })
    );
    assert!(api_is_color_scheme_report("\x1b[?997;1n"));
    assert_eq!(
        api_parse_color_scheme_report("\x1b[?997;1n"),
        Some(ApiTerminalColorScheme::Dark)
    );
}

#[test]
fn public_api_symbols_are_importable() {
    assert_eq!(visible_width("abc"), 3);

    let mut container = Container::new();
    container.add_child(Box::new(Text::new("hello")));
    container.add_child(Box::new(Spacer::new(1)));
    let lines = container.render(20);
    assert_eq!(lines, vec!["hello".to_string(), "".to_string()]);

    let terminal = VirtualTerminal::new(20, 5);
    let tui = Tui::new(terminal);
    assert_eq!(
        tui.terminal().size(),
        TerminalSize {
            columns: 20,
            rows: 5
        }
    );

    let mut loader = Loader::new("Loading");
    loader.tick();
    let _ = CancellableLoader::new(
        loader,
        KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()),
    );

    let mut panel = TuiBox::new();
    panel.add_child(std::boxed::Box::new(TruncatedText::new("Loading")));
    let _ = panel.render(20);

    let _ = SettingsList::new(
        vec![SettingItem::new("theme", "Theme", "dark")],
        5,
        KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()),
    );
    let _ = SettingsListOptions::default();

    let theme = light_theme();
    assert_eq!(theme.mode, ThemeMode::Light);
    let _ = TuiTheme::dark();
    let _ = Markdown::new("**hello**").with_theme(theme.markdown);

    let _ = SelectorDialog::new(
        "Model",
        vec![SelectItem::new("deepseek-v4-flash", "deepseek-v4-flash")],
        KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()),
        SelectorDialogOptions {
            theme: theme.select_list,
            ..Default::default()
        },
    );

    let capabilities = TerminalCapabilities {
        images: Some(ImageProtocol::Kitty),
        true_color: true,
        hyperlinks: true,
    };
    let _ = Image::new("abc", "image/png")
        .dimensions(ImageDimensions {
            width_px: 18,
            height_px: 18,
        })
        .capabilities(capabilities);
    let _ = ImageRenderOptions::default();
    let _ = CellDimensions::default();
    let _ = calculate_image_cell_size(
        ImageDimensions {
            width_px: 18,
            height_px: 18,
        },
        10,
        None,
        CellDimensions::default(),
    );
    let _ = detect_terminal_capabilities_from_env([("KITTY_WINDOW_ID", "1")], || false);
    let _ = encode_kitty("abc", Default::default());
    let _ = encode_iterm2("abc", Default::default());
    let _ = delete_kitty_image(1);
    let _ = delete_all_kitty_images();
    let _ = image_dimensions_from_bytes(&[], "image/png");
    assert!(is_image_line("\x1b_Ga=T;abc\x1b\\"));
    let _ = wrap_text_with_ansi("\x1b[31mhello world\x1b[0m", 8);
    let _ = truncate_to_width_with_ellipsis("abcdef", 4);

    let provider = pi_tui::api::input::CombinedAutocompleteProvider::new(
        vec![SlashCommand::new("model")],
        std::path::Path::new("."),
    );
    let _ = provider.get_suggestions(&["/m".to_string()], 0, 2, AutocompleteOptions::default());
    let _ = AutocompleteItem::new("model", "model");

    let _ = paint_with_level("x", &Style::fg(Color::Ansi256(1)), ColorLevel::Ansi256);
    let _ = detect_color_level_from_env([("TERM", "xterm-256color")]);
    let _ = color_level();

    let _ = std::mem::size_of::<ProcessTerminal>();
}
