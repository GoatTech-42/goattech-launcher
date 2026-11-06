//! A local client JAR is inspected first, then installed only on an explicit second click.
use super::*;
use horizon_launcher::client_jar::{self, ClientJar};
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct ClientInstall {
    pub(super) busy: bool,
    pub(super) chosen: Option<(PathBuf, ClientJar, String)>,
    pub(super) report: Option<String>,
    picker_error: Option<String>,
    show_picker_error: bool,
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
                self.report = Some(tr("The file picker is unavailable. On Linux, check that your desktop's file chooser portal and session bus are running, then try again.").to_string());
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
            prompt: Some(tr("Choose client JAR").into()),
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
                    return Err("This JAR is server-only, not a client mod.".to_string());
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
        let task = cx.background_executor().spawn(async move {
            let versions = horizon_launcher::minecraft_versions::load()?;
            client_jar::install_reviewed(
                &backend::game_home(),
                &backend::data_dir(),
                Some(&versions),
                &client_jar::Goal {
                    jar: &path,
                    instance: None,
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
                            "Installed {} {} in new instance {}: Minecraft {}, {}. {}",
                            done.jar.name,
                            done.jar.version,
                            done.instance,
                            done.minecraft,
                            done.loader,
                            done.notes.join(" ")
                        ));
                    }
                    Err(e) => this.instances.client.report = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn client_install_section(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let n = t.night;
        let busy = self.instances.client.busy;
        let mut section=div().flex().flex_col().gap(px(10.)).mt(px(26.))
            .child(shell::text(theme::night::CAPTION,shell::c(n.ink)).child(tr("Client JAR").to_string()))
            .child(shell::text(theme::night::CAPTION,shell::c(n.muted)).child(tr("Choose a local Fabric client mod from its own website. Inspect it here, then install it in a new Java instance. Existing instances stay unchanged.").to_string()))
            .child(shell::text(theme::night::CAPTION,shell::c(n.muted)).child(tr("Some clients and Fabric API bundles need dependency resolution not supported here yet. Client-specific config sharing is not available; options.txt sharing is separate.").to_string()));
        let label = if busy {
            "Working..."
        } else {
            "Choose client JAR"
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
            let mut install = button(
                "install-client-jar",
                if busy {
                    "Installing..."
                } else {
                    "Install in new instance"
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
                        "Hide technical details"
                    } else {
                        "Technical details"
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
            "This JAR is server-only, not a client mod.",
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
