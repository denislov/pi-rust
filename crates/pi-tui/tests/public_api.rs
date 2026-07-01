use pi_tui::api::{
    AutocompleteItem as ApiAutocompleteItem, Box as ApiBox, Component as ApiComponent,
    Editor as ApiEditor, InputEvent as ApiInputEvent, Key as ApiKey,
    KeybindingsManager as ApiKeybindingsManager, Markdown as ApiMarkdown,
    OverlayOptions as ApiOverlayOptions, ProcessTerminal as ApiProcessTerminal,
    RenderScheduler as ApiRenderScheduler, RenderStrategy as ApiRenderStrategy,
    Terminal as ApiTerminal, Text as ApiText, Tui as ApiTui, TuiTheme as ApiTuiTheme,
    VirtualTerminal as ApiVirtualTerminal,
};
use pi_tui::{
    AutocompleteItem, AutocompleteOptions, Box as TuiBox, CancellableLoader, CellDimensions,
    Container, Image, ImageDimensions, ImageProtocol, ImageRenderOptions, KeybindingsManager,
    Loader, Markdown, ProcessTerminal, SelectItem, SelectorDialog, SelectorDialogOptions,
    SettingItem, SettingsList, SettingsListOptions, SlashCommand, Spacer, TUI_KEYBINDINGS,
    TerminalCapabilities, TerminalSize, Text, ThemeMode, TruncatedText, Tui, TuiTheme,
    VirtualTerminal, calculate_image_cell_size, color_level, delete_all_kitty_images,
    delete_kitty_image, detect_color_level_from_env, detect_terminal_capabilities_from_env,
    encode_iterm2, encode_kitty, image_dimensions_from_bytes, is_image_line, light_theme,
    paint_with_level, truncate_to_width_with_ellipsis, visible_width, wrap_text_with_ansi,
};

#[test]
fn generic_tui_symbols_are_importable_from_api_facade() {
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
        _render_scheduler: Option<ApiRenderScheduler>,
        _render_strategy: Option<ApiRenderStrategy>,
        _text: Option<ApiText>,
        _theme: Option<ApiTuiTheme>,
        _virtual_terminal: Option<ApiVirtualTerminal>,
    ) {
    }

    accepts_types(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    );

    fn accepts_component<T: ApiComponent>() {}
    fn accepts_terminal<T: ApiTerminal>() {}
    let _ = accepts_component::<ApiText>;
    let _ = accepts_terminal::<ApiVirtualTerminal>;
    let _ = std::any::type_name::<ApiTui<ApiVirtualTerminal>>();
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

    let provider = pi_tui::CombinedAutocompleteProvider::new(
        vec![SlashCommand::new("model")],
        std::path::Path::new("."),
    );
    let _ = provider.get_suggestions(&["/m".to_string()], 0, 2, AutocompleteOptions::default());
    let _ = AutocompleteItem::new("model", "model");

    let _ = paint_with_level(
        "x",
        &pi_tui::Style::fg(pi_tui::Color::Ansi256(1)),
        pi_tui::ColorLevel::Ansi256,
    );
    let _ = detect_color_level_from_env([("TERM", "xterm-256color")]);
    let _ = color_level();

    let _ = std::mem::size_of::<ProcessTerminal>();
}
