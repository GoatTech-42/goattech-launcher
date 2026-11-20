//! A local client JAR is inspected first, then installed only on an explicit second click.
use super::*;
use horizon_launcher::client_jar::{self, ClientJar};
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct ClientInstall {
    pub(super) busy: bool,
    pub(super) chosen: Option<(PathBuf, ClientJar, String)>,
    pub(super) report: Option<String>,
    /// The existing instance (its id) the JAR goes into; none: a new instance.
    pub(super) target: Option<String>,
    picker_error: Option<String>,
    show_picker_error: bool,
}

/// The next install target when the player clicks the target button: new instance, then each
/// instance that can take a Fabric client JAR, then new again. An id no longer in the list
/// (the instance was removed) falls back to new.
fn next_target(ids: &[String], current: Option<&str>) -> Option<String> {
    match current {
        None => ids.first().cloned(),
        Some(id) => ids
            .iter()
            .position(|i| i == id)
            .and_then(|at| ids.get(at + 1))
            .cloned(),
    }
}

/// The target the install will really use: the chosen id only while it is still an eligible
/// instance, else none (a new instance). Render and install both go through this.
fn valid_target(eligible: &[(String, String)], chosen: Option<&str>) -> Option<(String, String)> {
    let id = chosen?;
    eligible.iter().find(|(i, _)| i == id).cloned()
}

impl ClientInstall {
    fn begin_pick(&mut self) {
        self.busy = true;
        self.report = None;
        self.picker_error = None;
        self.show_picker_error = false;
        self.chosen = None;
    }

    fn picked(&mut self, result: Option<Result<PathBuf, String>>) -> Option<PathBuf> {
        match result {
            Some(Ok(path)) => Some(path),
            Some(Err(e)) => {
                self.busy = false;
                self.report = Some(tr("the file picker isn't available. on linux, make sure your desktop's file chooser is running, then try again.").to_string());
                self.picker_error = Some(e);
                None
            }
            None => {
                self.busy = false;
                None
            }
        }
    }

    fn inspected(&mut self, result: Result<(PathBuf, ClientJar, String), String>) {
        self.busy = false;
        match result {
            Ok(chosen) => self.chosen = Some(chosen),
            Err(e) => self.report = Some(e),
        }
    }
}

impl Launcher {
    fn choose_client_jar(&mut self, cx: &mut Context<Self>) {
        if self.instances.client.busy {
            return;
        }
        self.instances.client.begin_pick();
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("choose client jar").into()),
        });
        cx.spawn(async move |this, cx| {
            let result = match paths.await {
                Ok(Ok(Some(mut paths))) => paths.pop().map(Ok),
                Ok(Ok(None)) => None,
                Ok(Err(e)) => Some(Err(e.to_string())),
                Err(e) => Some(Err(e.to_string())),
            };
            let path = this
                .update(cx, |this, cx| {
                    let path = this.instances.client.picked(result);
                    cx.notify();
                    path
                })
                .ok()
                .flatten();
            let Some(path) = path else {
                return;
            };
            let task = cx.background_executor().spawn(async move {
                let (info, hash) = client_jar::inspect_for_review(&path)?;
                if info.side == client_jar::Side::Server {
                    return Err("this jar is server-only, not a client mod.".to_string());
                }
                Ok::<_, String>((path, info, hash))
            });
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.instances.client.inspected(result);
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn install_chosen_client(&mut self, cx: &mut Context<Self>) {
        if self.instances.client.busy {
            return;
        }
        let Some((path, _, hash)) = self.instances.client.chosen.clone() else {
            return;
        };
        self.instances.client.busy = true;
        self.instances.client.report = None;
        let target = valid_target(
            &self.client_targets(),
            self.instances.client.target.as_deref(),
        )
        .map(|(id, _)| id);
        let task = cx.background_executor().spawn(async move {
            let versions = horizon_launcher::minecraft_versions::load()?;
            client_jar::install_reviewed(
                &backend::game_home(),
                &backend::data_dir(),
                Some(&versions),
                &client_jar::Goal {
                    jar: &path,
                    instance: target.as_deref(),
                    name: None,
                    minecraft: None,
                    loader_version: None,
                },
                &backend::get_bytes,
                Some(&hash),
            )
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.instances.client.busy = false;
                match result {
                    Ok(done) => {
                        this.reload_instances();
                        this.instances.client.chosen = None;
                        this.instances.client.report = Some(format!(
                            "installed {} {} in {} instance {}: minecraft {}, {}. {}",
                            done.jar.name,
                            done.jar.version,
                            if done.created { "new" } else { "your" },
                            done.instance,
                            done.minecraft,
                            done.loader,
                            done.notes.join(" ")
                        ));
                    }
                    Err(e) => this.instances.client.report = Some(client_jar::window_words(e)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Instances a Fabric client JAR can go into: Java edition, Fabric Loader set.
    fn client_targets(&self) -> Vec<(String, String)> {
        self.instances
            .list
            .iter()
            .filter(|i| {
                i.java
                    .as_ref()
                    .and_then(|j| j.loader.as_ref())
                    .is_some_and(|l| l.kind == horizon_launcher::loaders::Kind::Fabric)
            })
            .map(|i| (i.id.clone(), i.name.clone()))
            .collect()
    }

    pub(super) fn client_install_section(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let n = t.night;
        let busy = self.instances.client.busy;
        let mut section=div().flex().flex_col().gap(px(10.)).mt(px(26.))
            .child(shell::text(theme::night::CAPTION,shell::c(n.ink)).child(tr("client jar").to_string()))
            .child(shell::text(theme::night::CAPTION,shell::c(n.muted)).child(tr("choose a local fabric client mod from its own website. inspect it here, then install it in a new instance, or in one of your fabric instances with the into button.").to_string()))
            .child(shell::text(theme::night::CAPTION,shell::c(n.muted)).child(tr("some clients and fabric api bundles need extra files this launcher can't fetch yet. sharing a client's own settings isn't available. options.txt sharing is separate.").to_string()));
        let label = if busy {
            "working..."
        } else {
            "choose client jar"
        };
        let mut choose = button("choose-client-jar", label, Kind::Secondary, &t);
        if !busy {
            choose = choose
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.choose_client_jar(cx)));
        }
        section = section.child(div().flex().child(choose));
        if let Some((path, info, _)) = &self.instances.client.chosen {
            section = section.child(shell::text(theme::night::CAPTION, shell::c(n.ink)).child(
                format!(
                    "{} {} ({}) - {}",
                    info.name,
                    info.version,
                    info.mod_id,
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            ));
            let ids = self.client_targets();
            let chosen = valid_target(&ids, self.instances.client.target.as_deref());
            let target_name = chosen.map(|(_, name)| name);
            let mut pick = button(
                "client-jar-target",
                &match &target_name {
                    Some(name) => format!("into: {name}"),
                    None => "into: a new instance".to_string(),
                },
                Kind::Secondary,
                &t,
            );
            if !busy && !ids.is_empty() {
                pick = pick.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    let ids: Vec<String> =
                        this.client_targets().into_iter().map(|(id, _)| id).collect();
                    let now = this.instances.client.target.clone();
                    this.instances.client.target = next_target(&ids, now.as_deref());
                    cx.notify();
                }));
            }
            section = section.child(div().flex().child(pick));
            let mut install = button(
                "install-client-jar",
                if busy {
                    "installing..."
                } else if target_name.is_some() {
                    "install in this instance"
                } else {
                    "install in new instance"
                },
                Kind::Secondary,
                &t,
            );
            if !busy {
                install = install.on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| this.install_chosen_client(cx)),
                );
            }
            section = section.child(div().flex().child(install));
        }
        if let Some(report) = &self.instances.client.report {
            section = section
                .child(shell::text(theme::night::CAPTION, shell::c(n.ink)).child(report.clone()));
        }
        if self.instances.client.picker_error.is_some() {
            section = section.child(
                button(
                    "client-picker-details",
                    if self.instances.client.show_picker_error {
                        "hide technical details"
                    } else {
                        "technical details"
                    },
                    Kind::Ghost,
                    &t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.instances.client.show_picker_error =
                        !this.instances.client.show_picker_error;
                    cx.notify();
                })),
            );
            if self.instances.client.show_picker_error {
                section = section.child(
                    text(theme::CAPTION).text_color(t.muted).child(
                        self.instances
                            .client
                            .picker_error
                            .clone()
                            .unwrap_or_default(),
                    ),
                );
            }
        }
        section
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_removed_or_ineligible_target_is_not_handed_to_the_installer() {
        let eligible = vec![("a".to_string(), "Alpha".to_string())];
        assert_eq!(
            valid_target(&eligible, Some("a")),
            Some(("a".to_string(), "Alpha".to_string()))
        );
        assert_eq!(valid_target(&eligible, Some("gone")), None);
        assert_eq!(valid_target(&[], Some("a")), None);
        assert_eq!(valid_target(&eligible, None), None);
    }

    #[test]
    fn target_button_walks_new_then_each_instance_then_new() {
        let ids = vec!["a".to_string(), "b".to_string()];
        assert_eq!(next_target(&ids, None).as_deref(), Some("a"));
        assert_eq!(next_target(&ids, Some("a")).as_deref(), Some("b"));
        assert_eq!(next_target(&ids, Some("b")), None);
        assert_eq!(next_target(&ids, Some("gone")), None);
        assert_eq!(next_target(&[], None), None);
    }

    #[test]
    fn picker_failure_retains_hidden_detail_and_retry_cancel_resets_it() {
        let mut state = ClientInstall::default();
        state.begin_pick();
        assert!(state.busy);
        assert!(state
            .picked(Some(Err("ZBus transport fixture".into())))
            .is_none());
        assert!(!state.busy);
        assert!(state.report.as_ref().unwrap().contains("file picker"));
        assert!(!state.report.as_ref().unwrap().contains("ZBus"));
        assert_eq!(
            state.picker_error.as_deref(),
            Some("ZBus transport fixture")
        );
        assert!(!state.show_picker_error);
        state.show_picker_error = true;
        state.begin_pick();
        assert!(state.picker_error.is_none());
        assert!(!state.show_picker_error);
        assert!(state.report.is_none());
        assert!(state.picked(None).is_none());
        assert!(!state.busy);
        assert!(state.report.is_none());
        assert!(state.picker_error.is_none());
    }

    #[test]
    fn parser_and_server_only_errors_do_not_become_transport_failures() {
        for error in [
            "invalid fabric.mod.json",
            "this jar is server-only, not a client mod.",
        ] {
            let mut state = ClientInstall::default();
            state.begin_pick();
            assert_eq!(
                state.picked(Some(Ok(PathBuf::from("fixture.jar")))),
                Some(PathBuf::from("fixture.jar"))
            );
            assert!(state.busy);
            state.inspected(Err(error.to_string()));
            assert!(!state.busy);
            assert_eq!(state.report.as_deref(), Some(error));
            assert!(state.picker_error.is_none());
            assert!(!state.show_picker_error);
            assert!(state.chosen.is_none());
        }
    }
}
