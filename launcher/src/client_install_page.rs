//! A local client JAR is inspected first, then installed only on an explicit second click.
use super::*;
use horizon_launcher::client_jar::{self, ClientJar};
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct ClientInstall {
    pub(super) busy: bool,
    pub(super) chosen: Option<(PathBuf, ClientJar, String)>,
    pub(super) report: Option<String>,
}

impl Launcher {
    fn choose_client_jar(&mut self, cx: &mut Context<Self>) {
        if self.instances.client.busy {
            return;
        }
        self.instances.client.busy = true;
        self.instances.client.report = None;
        self.instances.client.chosen = None;
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
            let Some(result) = result else {
                let _ = this.update(cx, |this, cx| {
                    this.instances.client.busy = false;
                    cx.notify();
                });
                return;
            };
            let task = cx.background_executor().spawn(async move {
                let path = result?;
                let (info, hash) = client_jar::inspect_for_review(&path)?;
                if info.side == client_jar::Side::Server {
                    return Err("This JAR is server-only, not a client mod.".to_string());
                }
                Ok::<_, String>((path, info, hash))
            });
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.instances.client.busy = false;
                match result {
                    Ok(chosen) => this.instances.client.chosen = Some(chosen),
                    Err(e) => this.instances.client.report = Some(e),
                }
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
        section
    }
}
