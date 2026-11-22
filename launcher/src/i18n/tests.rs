use super::*;
use std::collections::BTreeSet;
use std::path::Path;

/// Every Rust file of the launcher's sources.
fn sources(dir: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(std::fs::read_to_string(path).unwrap());
        }
    }
}

/// Read Rust tokens rather than quote positions: whitespace, comments, escaped and raw strings
/// must not hide a call or turn a quoted example into one. Recurse into macro bodies too.
fn strings_in(
    tokens: proc_macro2::TokenStream,
    asked: &mut BTreeSet<String>,
    all: &mut BTreeSet<String>,
) {
    use proc_macro2::{Delimiter, TokenTree};
    let mut tokens = tokens.into_iter().peekable();
    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Ident(name) if name == "tr" || name == "trf" => {
                if let Some(TokenTree::Group(args)) = tokens.peek() {
                    if args.delimiter() == Delimiter::Parenthesis {
                        if let Some(TokenTree::Literal(value)) = args.stream().into_iter().next() {
                            if let Ok(value) = syn::parse_str::<syn::LitStr>(&value.to_string()) {
                                asked.insert(value.value());
                            }
                        }
                    }
                }
            }
            TokenTree::Group(group) => strings_in(group.stream(), asked, all),
            TokenTree::Literal(value) => {
                if let Ok(value) = syn::parse_str::<syn::LitStr>(&value.to_string()) {
                    all.insert(value.value());
                }
            }
            _ => {}
        }
    }
}

/// The English strings requested directly, plus every literal (some reach `tr` via a widget or
/// constant). The tables' shared key set below covers those indirect translations.
fn strings() -> (BTreeSet<String>, BTreeSet<String>) {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let (mut asked, mut all) = (BTreeSet::new(), BTreeSet::new());
    for file in &files {
        strings_in(
            file.parse().expect("valid Rust tokens"),
            &mut asked,
            &mut all,
        );
    }
    (asked, all)
}

#[test]
fn the_scan_finds_multiline_calls_without_reading_comments_or_quoted_code() {
    let code = r###"
        tr
        (
            "multiline"
        );
        trf(/* before the string */ "hello {name}", &[("name", "player")]);
        some_macro!(crate::i18n::tr(r#"raw "quotes""#));
        tr("escaped \"quotes\" and \u{4e2d}\nnext");
        tr("continued \
            line");
        // tr("line comment");
        /* tr("block comment"); */
        let example = r#"tr("quoted code")"#;
        other_tr("not a translation");
        trf_other("not a translation either");
        let indirect = "widget label";
        tr(indirect);
    "###;
    let (mut asked, mut all) = (BTreeSet::new(), BTreeSet::new());
    strings_in(code.parse().unwrap(), &mut asked, &mut all);
    assert_eq!(
        asked,
        [
            "multiline",
            "hello {name}",
            "raw \"quotes\"",
            "escaped \"quotes\" and 中\nnext",
            "continued line",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    );
    assert!(all.contains("widget label"));
    assert!(asked.is_subset(&all));
    assert!(!all.contains("line comment"));
    assert!(!all.contains("block comment"));
}

fn holes(s: &str) -> BTreeSet<&str> {
    s.split('{')
        .skip(1)
        .filter_map(|part| part.split_once('}').map(|(name, _)| name))
        .collect()
}

#[test]
fn every_language_has_every_string_the_code_asks_for() {
    let (mut asked, all) = strings();
    assert!(asked.contains("language"), "the scan finds tr(\"…\")");
    let tables: Vec<_> = Language::ALL
        .into_iter()
        .filter(|language| *language != Language::English)
        .map(|language| {
            let table: HashMap<String, String> = serde_json::from_str(language.source())
                .unwrap_or_else(|e| panic!("lang/{}.json: {e}", language.code()));
            (language, table)
        })
        .collect();
    // A label passed through a widget or a constant is not a literal argument to `tr`. Once
    // translated in any language it must exist in all of them, not silently fall back to English.
    for (_, table) in &tables {
        asked.extend(table.keys().cloned());
    }
    for (language, table) in &tables {
        let missing: Vec<_> = asked.iter().filter(|s| !table.contains_key(*s)).collect();
        assert!(
            missing.is_empty(),
            "lang/{}.json lacks {missing:#?}",
            language.code()
        );
        let unused: Vec<_> = table.keys().filter(|s| !all.contains(*s)).collect();
        assert!(
            unused.is_empty(),
            "lang/{}.json has strings the code no longer has: {unused:#?}",
            language.code()
        );
        for (en, said) in table {
            assert_eq!(holes(en), holes(said), "{}: {en:?}", language.code());
            assert!(
                !said.trim().is_empty(),
                "{}: {en:?} is empty",
                language.code()
            );
        }
    }
}

#[test]
fn the_system_language_is_the_first_one_the_launcher_has() {
    assert_eq!(Language::pick(["pl-PL", "en-US"]), Language::Polish);
    assert_eq!(Language::pick(["ja-JP", "en-US"]), Language::Japanese);
    assert_eq!(Language::parse("ja_JP.UTF-8"), Some(Language::Japanese));
    assert_eq!(Language::pick(["tr-TR", "en-US"]), Language::Turkish);
    assert_eq!(Language::parse("tr_TR.UTF-8"), Some(Language::Turkish));
    assert_eq!(Language::parse("pl_PL.UTF-8"), Some(Language::Polish));
    assert_eq!(Language::pick(["fr-CH", "en-US"]), Language::French);
    assert_eq!(Language::pick(["it-IT", "ru_RU.UTF-8"]), Language::Russian);
    assert_eq!(Language::pick(["it-IT", "zh-Hans-CN"]), Language::Chinese);
    assert_eq!(Language::pick(["it-IT", "ja"]), Language::Japanese);
    assert_eq!(Language::pick(["de-CH", "ru_RU.UTF-8"]), Language::German);
    assert_eq!(Language::pick(["de-DE", "ja"]), Language::German);
    assert_eq!(Language::pick(["de-DE", "zh-Hans-CN"]), Language::German);
    assert_eq!(Language::pick(["es-ES", "en-US"]), Language::Spanish);
    assert_eq!(
        Language::pick(["pt-BR", "en-US"]),
        Language::PortugueseBrazil
    );
    assert_eq!(Language::pick(["ko", "it-IT"]), Language::English);
    assert_eq!(Language::pick(["uk-UA", "ru-RU"]), Language::Ukrainian);
    assert_eq!(Language::parse("uk_UA.UTF-8"), Some(Language::Ukrainian));
    assert_eq!(
        Language::parse("pt_BR.UTF-8"),
        Some(Language::PortugueseBrazil)
    );
    assert_eq!(Language::parse("pt-BR"), Some(Language::PortugueseBrazil));
    assert_eq!(Language::parse("pt-PT"), None); // Explicit check: pt-PT won't alias pt-BR
    assert_eq!(Language::pick([]), Language::English);
    assert_eq!(Language::parse("FR"), Some(Language::French));
    assert_eq!(Language::parse("french"), None);
    assert_eq!(Language::parse("DE"), Some(Language::German));
    assert_eq!(Language::parse("ES"), Some(Language::Spanish));
    assert_eq!(Language::parse("german"), None);
    assert_eq!(Language::parse("ZH"), Some(Language::Chinese));
    assert_eq!(serde_json::to_string(&Language::German).unwrap(), "\"de\"");
    assert_eq!(serde_json::to_string(&Language::Chinese).unwrap(), "\"zh\"");
    assert_eq!(serde_json::to_string(&Language::Russian).unwrap(), "\"ru\"");
    assert_eq!(serde_json::to_string(&Language::Spanish).unwrap(), "\"es\"");
    assert_eq!(
        serde_json::to_string(&Language::PortugueseBrazil).unwrap(),
        "\"pt-br\""
    );
}

#[test]
fn a_missing_string_shows_in_english() {
    assert_eq!(tr_in(Language::Polish, "settings"), "Ustawienia");
    assert_eq!(tr_in(Language::Japanese, "settings"), "設定");
    assert_eq!(tr_in(Language::Turkish, "settings"), "Ayarlar");
    assert_eq!(tr_in(Language::French, "settings"), "Réglages");
    assert_eq!(tr_in(Language::Russian, "settings"), "Настройки");
    assert_eq!(tr_in(Language::German, "settings"), "Optionen");
    assert_eq!(tr_in(Language::Chinese, "settings"), "设置");
    assert_eq!(tr_in(Language::Spanish, "settings"), "Ajustes");
    assert_eq!(tr_in(Language::Ukrainian, "settings"), "Параметри");
    assert_eq!(tr_in(Language::English, "settings"), "settings");
    assert_eq!(tr_in(Language::French, "No such string"), "No such string");
    assert_eq!(tr_in(Language::PortugueseBrazil, "settings"), "Ajustes");
}

#[test]
fn polish_entries_are_valid_and_keep_placeholders() {
    let (_, all) = strings();
    let table: HashMap<String, String> = serde_json::from_str(Language::Polish.source()).unwrap();
    for (en, said) in &table {
        assert!(all.contains(en), "unused Polish string: {en:?}");
        assert!(!said.trim().is_empty(), "empty Polish string: {en:?}");
        assert_eq!(holes(en), holes(said), "Polish: {en:?}");
    }
}

/// Consent's complete rendering scope ignores every translation, even in shared widgets.
#[test]
fn consent_is_fully_english_but_the_rest_of_the_ui_stays_translated() {
    let keys = [
        "after each session: frame-time percentiles and a 14-bucket histogram, tick times, peak memory and startup times, with your machine (os, cpu and its cores and clocks, memory size, gpu, its memory and driver, drive type, screen size and refresh rate, laptop or desktop, on battery or not) and the settings that change performance.",
        "bug reports",
        "crash reports",
        "data collection",
        "help make GoatTech faster and steadier?",
        "hide details and example reports",
        "only the reports you write and send yourself, with the game's log if you add it after reading it.",
        "performance statistics",
        "save choices",
        "the results of benchmarks you run, for the community results on playhorizon.dev.",
        "the short version: on a new install, basic performance summaries start on; the four report kinds start off. \"keep optional data off\" turns all five off in one click - the anonymous player-count ping runs while you play either way. settings can change any choice later.",
        "the short version: on a new install, basic performance summaries start on; the four report kinds start off. \"keep optional data off\" turns all five off in one click - the player-count ping is off in this configuration. settings can change any choice later.",
        "upload this log to mclo.gs, a paste service for minecraft logs? anyone with the link can read it, until it is deleted or unread for 90 days. your home folder, user name and account names are taken out first.",
        "view details and example reports",
        "when the game crashes: the error, where in horizon's code it happened, your graphics adapter and settings. no log, no world or server, no user name in paths.",
        "your data",
        "{count} waiting to be sent",
    ];
    for language in Language::ALL {
        let outside = tr_in(language, "data collection").to_owned();
        {
            let _english = english_only();
            for en in keys {
                assert_eq!(tr_in(language, en), en, "{}: {en:?}", language.code());
            }
            {
                let _nested = english_only();
                assert_eq!(tr_in(language, "save choices"), "save choices");
            }
            assert_eq!(tr_in(language, "save choices"), "save choices");
        }
        assert_eq!(tr_in(language, "data collection"), outside);
    }
    assert_ne!(tr_in(Language::German, "data collection"), "data collection");
    let source = include_str!("../collection.rs");
    for signature in [
        "fn consent_screen(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {",
        "fn collection_section(&mut self, cx: &mut Context<Self>) -> Div {",
    ] {
        let body = source.split(signature).nth(1).unwrap();
        assert!(body.trim_start().starts_with("let _english ="));
    }
}

/// The game-launch option wording and the "left off" note (mods an import omitted) read as the
/// launcher's own language in Turkish and Japanese, not English. (Machine-translated; a native
/// speaker has not reviewed them.)
#[test]
fn launch_option_texts_and_the_left_off_note_are_translated_in_turkish_and_japanese() {
    let keys = [
        "runs {command} first",
        "runs {command} after",
        "through {wrapper}",
        "run the game through",
        "a shell command, run in the instance's folder once the game has exited.",
        "a shell command, run in the instance's folder. the game starts only if it succeeds.",
        "left off: {line}.",
    ];
    for language in [Language::Turkish, Language::Japanese] {
        for en in keys {
            assert_ne!(tr_in(language, en), en, "{}: {en:?}", language.code());
        }
    }
}

/// The AI and agent texts, including the switch that turns every AI feature off, are shown in
/// Turkish, Japanese and German, not English. (Machine-translated; no native speaker has reviewed
/// them.)
#[test]
fn the_ai_and_agent_texts_are_translated_in_turkish_japanese_and_german() {
    let keys = [
        "any mcp client",
        "connect an ai agent",
        "hides and turns off every ai feature: agent control of the launcher and the game, horizon-ctl and the mcp servers.",
        "no ai",
        "on: every ai feature is hidden and off. the game opens no control socket, and horizon-ctl and the mcp servers refuse.",
        "the mcp server answered with {count} tools. now ask your agent: \"say hello from horizon\".",
        "the mcp server did not answer: {error}",
        "what an agent can do, and the commands",
        "your own agent (claude code, codex, cursor or any mcp client) can drive the launcher and the game it starts, with the same commands as the terminal. it runs on your computer and talks to horizon there; it asks before deleting anything.",
        "{client} adds it with this command (it runs only when you click):",
    ];
    for language in [Language::Turkish, Language::Japanese, Language::German] {
        for en in keys {
            assert_ne!(tr_in(language, en), en, "{}: {en:?}", language.code());
        }
    }
}

/// The launch-option and pack-added notes are shown in Turkish and Japanese, not English.
/// (Machine-translated; no native speaker has reviewed them.)
#[test]
fn the_launch_option_and_pack_notes_are_translated_in_turkish_and_japanese() {
    let keys = [
        "on the dedicated gpu",
        "dedicated graphics card",
        "with mangohud",
        "with gamemode",
        "when it starts",
        "with 1 variable",
        "with {count} variables",
        "for example nice -n 5",
        "for example ./backup.sh",
        "for example name=value other=\"two words\"",
        "{count} lines, the game is running",
        "launcher updated to {version}",
        "added {names}: turn it on below.",
        "added {names}: the game turns it on when the world opens.",
        "added {title} to {instance}'s packs: switch it on under packs.",
    ];
    for language in [Language::Turkish, Language::Japanese] {
        for en in keys {
            assert_ne!(tr_in(language, en), en, "{}: {en:?}", language.code());
        }
    }
}

/// The import, update-check, load-error and instance texts are shown in German, not English.
/// (Machine-translated; no native speaker has reviewed them.)
#[test]
fn the_import_update_and_load_error_texts_are_translated_in_german() {
    let keys = [
        "add to {client}",
        "install id {prefix}…{suffix}",
        "jars: {summary}.",
        "port gets the jars of the {count} mods this computer can port today (content with data and assets only) from the pack's own links, modrinth, or your folder, and carries them over. the ports stay here, in {folder}.",
        "use these jars",
        "use jars from my instance…",
        "mods: {mods}",
        "only on curseforge (curseforge downloads coming soon: choose your installed instance instead)",
        "port {count} mods",
        "copied from your folder",
        "launcher updated to {version}",
        "no build for this platform",
        "no notes for this release.",
        "no version",
        "signed in as {name}.",
        "couldn't check for updates. check the connection and try again in a few minutes.",
        "mojang's version list could not be fetched. check the connection and try again in a few minutes.",
        "the mod registry can't be reached. check the connection and try again in a few minutes.",
        "choose client jar",
        "client jar",
        "install in new instance",
        "hide technical details",
        "technical details",
        "couldn't load releases. check the connection and try again in a few minutes.",
        "couldn't load patch notes. check the connection and try again in a few minutes.",
    ];
    for en in keys {
        assert_ne!(tr_in(Language::German, en), en, "{en:?}");
    }
}

/// The Java runtime, update-check, load-error and instance texts are shown in Turkish and
/// Japanese, not English. (Machine-translated; no native speaker has reviewed them.)
#[test]
fn the_java_update_and_load_error_texts_are_translated_in_turkish_and_japanese() {
    let keys = [
        "install to",
        "each java instance gets the runtime its version needs, downloaded once and reused where versions match.",
        "each java instance gets the runtime its version needs, downloaded once and reused where versions match. {downloaded} downloaded · {installed} on this computer.",
        "each java instance gets the runtime its version needs. none downloaded yet - mojang's is downloaded when an instance needs it.",
        "java runtimes",
        "{n} mods",
        "couldn't check for updates. check the connection and try again in a few minutes.",
        "mojang's version list could not be fetched. check the connection and try again in a few minutes.",
        "the mod registry can't be reached. check the connection and try again in a few minutes.",
        "choose client jar",
        "client jar",
        "install in new instance",
        "hide technical details",
        "technical details",
        "couldn't load releases. check the connection and try again in a few minutes.",
        "couldn't load patch notes. check the connection and try again in a few minutes.",
    ];
    for language in [Language::Turkish, Language::Japanese] {
        for en in keys {
            assert_ne!(tr_in(language, en), en, "{}: {en:?}", language.code());
        }
    }
}

/// The include-list, Funnel, landscape and server-form texts are shown in Turkish and
/// Japanese, not English. (Machine-translated; no native speaker has reviewed them.)
#[test]
fn the_include_funnel_and_server_form_texts_are_translated_in_turkish_and_japanese() {
    let keys = [
        ", {count} listed by their modrinth download",
        "settings, packs and horizon mods go in; logs, screenshots, backups and your account never do. worlds go without their per-player files.",
        "game builds, servers made in the launcher, settings, skins.",
        "GoatTech never turns on Funnel: the server isn't put on the public internet, only shared with the people you invite.",
        "GoatTech's landscape. pick a png, jpeg or webp picture of your own.",
        "players allowed in, separated by commas (you first)",
        "seed (optional: leave empty for a random world)",
    ];
    for language in [Language::Turkish, Language::Japanese] {
        for en in keys {
            assert_ne!(tr_in(language, en), en, "{}: {en:?}", language.code());
        }
    }
}

/// The client-jar, bundle-limits and file-picker texts are shown in Turkish, Japanese and German,
/// not English, and German's two longest sidebar names are short enough for the 64px rail
/// (they broke mid-word at 10, 13 and 16 letters). (Machine-translated; no native review.)
#[test]
fn the_client_jar_texts_are_translated_and_german_rail_names_are_short() {
    let keys = [
        "choose a local fabric client mod from its own website. inspect it here, then install it in a new instance, or in one of your fabric instances with the into button.",
        "some clients and fabric api bundles need extra files this launcher can't fetch yet. sharing a client's own settings isn't available. options.txt sharing is separate.",
        "the file picker isn't available. on linux, make sure your desktop's file chooser is running, then try again.",
    ];
    for language in [Language::Turkish, Language::Japanese, Language::German] {
        for en in keys {
            assert_ne!(tr_in(language, en), en, "{}: {en:?}", language.code());
        }
    }
    for key in ["settings", "patch notes"] {
        let word = tr_in(Language::German, key);
        assert!(
            word.chars().count() <= 9,
            "{word:?} is too long for the rail"
        );
    }
}

/// The retry and details buttons and the "from other launchers" heading are translated in every
/// language, not English. (Machine-translated; no native speaker has reviewed them.)
#[test]
fn the_retry_and_details_buttons_are_translated_in_every_language() {
    for language in Language::ALL {
        if matches!(language, Language::English) {
            continue;
        }
        for en in [
            "try again",
            "details",
            "hide details",
            "from other launchers",
        ] {
            assert_ne!(tr_in(language, en), en, "{}: {en:?}", language.code());
        }
    }
}

/// The five load-error sentences are translated in every language, not English, and Polish's
/// sidebar name for patch notes is short enough for the 64px rail (the long form wrapped
/// mid-word and ran over the next item). (Machine-translated; no native speaker has reviewed them.)
#[test]
fn the_load_error_sentences_are_translated_in_every_language_and_polish_rail_name_is_short() {
    for language in Language::ALL {
        if matches!(language, Language::English) {
            continue;
        }
        for en in [
            "couldn't check for updates. check the connection and try again in a few minutes.",
            "mojang's version list could not be fetched. check the connection and try again in a few minutes.",
            "the mod registry can't be reached. check the connection and try again in a few minutes.",
            "couldn't load releases. check the connection and try again in a few minutes.",
            "couldn't load patch notes. check the connection and try again in a few minutes.",
        ] {
            assert_ne!(tr_in(language, en), en, "{}: {en:?}", language.code());
        }
    }
    let word = tr_in(Language::Polish, "patch notes");
    assert!(
        word.chars().count() <= 9,
        "{word:?} is too long for the rail"
    );
}

/// French, Brazilian Portuguese and Ukrainian sidebar names for settings and patch notes have no
/// word longer than 9 letters, so none breaks mid-word in the 64px rail (the old long forms did).
#[test]
fn french_portuguese_and_ukrainian_rail_names_have_short_words() {
    for language in [
        Language::French,
        Language::PortugueseBrazil,
        Language::Ukrainian,
    ] {
        for key in ["settings", "patch notes"] {
            for word in tr_in(language, key).split_whitespace() {
                assert!(
                    word.chars().count() <= 9,
                    "{}: {word:?} is too long for the rail",
                    language.code()
                );
            }
        }
    }
}
