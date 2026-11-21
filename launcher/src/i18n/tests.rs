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
    assert_eq!(tr_in(Language::French, "settings"), "Paramètres");
    assert_eq!(tr_in(Language::Russian, "settings"), "Настройки");
    assert_eq!(tr_in(Language::German, "settings"), "Einstellungen");
    assert_eq!(tr_in(Language::Chinese, "settings"), "设置");
    assert_eq!(tr_in(Language::Spanish, "settings"), "Ajustes");
    assert_eq!(tr_in(Language::Ukrainian, "settings"), "Налаштування");
    assert_eq!(tr_in(Language::English, "settings"), "settings");
    assert_eq!(tr_in(Language::French, "No such string"), "No such string");
    assert_eq!(
        tr_in(Language::PortugueseBrazil, "settings"),
        "Configurações"
    );
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

/// The data-collection and upload texts a player decides on are shown in Turkish, Japanese and
/// German, not English. (Machine-translated; a native speaker has not reviewed them.)
#[test]
fn the_data_collection_texts_are_translated_in_turkish_japanese_and_german() {
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
    for language in [Language::Turkish, Language::Japanese, Language::German] {
        for en in keys {
            assert_ne!(tr_in(language, en), en, "{}: {en:?}", language.code());
        }
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
