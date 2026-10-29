use super::*;

fn can_split_at(clip: &Clip, frame: f64, playhead: f64) -> bool {
    playhead - clip.timeline_start >= frame - 0.0001
        && clip.timeline_end() - playhead >= frame - 0.0001
}

fn can_trim_at(clip: &Clip, frame: f64, playhead: f64, audio: bool, before: bool) -> bool {
    let offset = playhead - clip.timeline_start;
    if !audio {
        // The decoder resolves video trim points (including the next VFR
        // timestamp for Trim after). Nominal FPS cannot predict that boundary.
        offset >= 0.0 && offset < clip.duration() - 0.000001 && (!before || offset > 0.000001)
    } else if before {
        offset > 0.000001 && offset <= clip.duration() - frame + 0.0001
    } else {
        offset >= frame - 0.0001 && offset < clip.duration() - 0.000001
    }
}

fn select_menu_target(
    clip: &Clip,
    audio: bool,
    playhead: &mut f64,
    library_preview: &mut bool,
    selected: &mut Option<ClipId>,
    selected_audio: &mut Option<ClipId>,
) {
    if *library_preview {
        *playhead = clip.timeline_start;
    }
    *library_preview = false;
    *selected_audio = audio.then_some(clip.id);
    if !audio {
        *selected = Some(clip.id);
    }
}

pub(super) fn context_menu(
    response: &egui::Response,
    keyboard: bool,
    keyboard_menu: &mut Option<(egui::Id, Pos2)>,
) -> egui::Popup<'static> {
    let menu_id = egui::Popup::default_response_id(response);
    let mut menu = egui::Popup::context_menu(response);
    if response.secondary_clicked() {
        *keyboard_menu = None;
    }
    if keyboard {
        *keyboard_menu = Some((menu_id, response.rect.center()));
        menu = menu.open_memory(Some(egui::SetOpenCommand::Bool(true)));
    }
    if let Some((id, position)) = *keyboard_menu
        && id == menu_id
    {
        menu = menu.at_position(position);
    }
    menu
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClipAction {
    Split,
    TrimBefore,
    TrimAfter,
}

fn edit_controls(
    ui: &mut egui::Ui,
    enabled: [bool; 3],
    controls: &mut BTreeMap<&'static str, Rect>,
) -> Option<ClipAction> {
    let mut action = None;
    for ((candidate, label, key, control), enabled) in [
        (ClipAction::Split, "Split", "S", "clip-menu-split"),
        (
            ClipAction::TrimBefore,
            "Trim before",
            "Q",
            "clip-menu-trim-before",
        ),
        (
            ClipAction::TrimAfter,
            "Trim after",
            "W",
            "clip-menu-trim-after",
        ),
    ]
    .into_iter()
    .zip(enabled)
    {
        let response = ui
            .add_enabled(enabled, egui::Button::new(label).shortcut_text(key))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(if candidate == ClipAction::Split {
                "Split at the playhead"
            } else {
                "Trim at the playhead"
            })
            .on_disabled_hover_text("Place the playhead inside this item");
        controls.insert(control, response.rect);
        if response.clicked() {
            action = Some(candidate);
            ui.close();
        }
    }
    action
}

impl EditorApp {
    fn can_split_clip(&self, clip: &Clip) -> bool {
        self.editor
            .metadata()
            .sources
            .iter()
            .find(|s| s.id == clip.source)
            .is_some_and(|s| can_split_at(clip, s.rate.frame_seconds(), self.playhead))
    }
    pub(super) fn can_split(&self) -> bool {
        if self.library_preview {
            return false;
        }
        if self.selected_audio.is_some() {
            return self
                .selected_clip()
                .is_some_and(|c| self.can_split_clip(&c));
        }
        self.timeline()
            .source_at(self.playhead)
            .and_then(|(id, _, _)| self.timeline().clips().iter().find(|c| c.id == id))
            .is_some_and(|c| self.can_split_clip(c))
    }
    pub(super) fn can_trim_selected(&self, before: bool) -> bool {
        if self.library_preview {
            return false;
        }
        self.selected_clip().is_some_and(|c| {
            self.editor
                .metadata()
                .sources
                .iter()
                .find(|s| s.id == c.source)
                .is_some_and(|s| {
                    can_trim_at(
                        &c,
                        s.rate.frame_seconds(),
                        self.playhead,
                        self.selected_audio.is_some(),
                        before,
                    )
                })
        })
    }
    pub(super) fn prepare_clip_menu(&mut self, clip: &Clip, audio: bool) {
        select_menu_target(
            clip,
            audio,
            &mut self.playhead,
            &mut self.library_preview,
            &mut self.selected,
            &mut self.selected_audio,
        );
        self.playing = false;
        self.scrub = None;
        self.settling = None;
        self.stepping = None;
        self.request_key.clear();
    }
    pub(super) fn clip_edit_menu(&mut self, ui: &mut egui::Ui) {
        let split = self
            .selected_clip()
            .is_some_and(|c| self.can_split_clip(&c));
        match edit_controls(
            ui,
            [
                split,
                self.can_trim_selected(true),
                self.can_trim_selected(false),
            ],
            &mut self.controls,
        ) {
            Some(ClipAction::Split) => self.split(),
            Some(ClipAction::TrimBefore) => self.quick_trim(true),
            Some(ClipAction::TrimAfter) => self.quick_trim(false),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Clip {
        Clip {
            id: ClipId(2),
            source: SourceId(1),
            source_in: 3.0,
            source_out: 5.0,
            timeline_start: 7.0,
        }
    }

    #[test]
    fn trim_eligibility_leaves_video_frame_boundaries_to_the_decoder() {
        let clip = target();
        let frame = 1.0 / 30.0;
        for audio in [false, true] {
            for before in [false, true] {
                for time in [6.99, 9.0, 9.01, f64::NAN] {
                    assert!(!can_trim_at(&clip, frame, time, audio, before));
                }
                assert!(can_trim_at(&clip, frame, 8.0, audio, before));
            }
        }
        assert!(!can_trim_at(&clip, frame, 7.0, false, true));
        assert!(can_trim_at(&clip, frame, 7.0, false, false));
        // These positions are less than a nominal frame from an endpoint.
        // A VFR seek/step may resolve a valid edit; do not reject it in the UI.
        assert!(can_trim_at(&clip, frame, 8.99, false, true));
        assert!(can_trim_at(&clip, frame, 8.99, false, false));
        assert!(!can_trim_at(&clip, frame, 8.99, true, true));
        assert!(!can_trim_at(&clip, frame, 7.01, true, false));
        assert!(can_trim_at(&clip, frame, 9.0 - frame, true, true));
        assert!(can_trim_at(&clip, frame, 7.0 + frame, true, false));
        assert!(!can_split_at(&clip, frame, 7.01));
        assert!(!can_split_at(&clip, frame, 8.99));
        assert!(can_split_at(&clip, frame, 7.0 + frame));
        assert!(can_split_at(&clip, frame, 9.0 - frame));
    }

    struct MenuHarness {
        ctx: egui::Context,
        clip: Clip,
        audio: bool,
        playhead: f64,
        library_preview: bool,
        selected: Option<ClipId>,
        selected_audio: Option<ClipId>,
        keyboard_menu: Option<(egui::Id, Pos2)>,
        response: Option<egui::Response>,
        controls: BTreeMap<&'static str, Rect>,
        actions: Vec<ClipAction>,
    }
    impl MenuHarness {
        fn new(audio: bool) -> Self {
            let ctx = egui::Context::default();
            theme::configure(&ctx);
            ctx.enable_accesskit();
            Self {
                ctx,
                clip: target(),
                audio,
                playhead: 8.0,
                library_preview: false,
                selected: Some(ClipId(99)),
                selected_audio: (!audio).then_some(ClipId(98)),
                keyboard_menu: None,
                response: None,
                controls: BTreeMap::new(),
                actions: Vec::new(),
            }
        }
        fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
            let ctx = self.ctx.clone();
            let modifiers = events
                .iter()
                .find_map(|event| match event {
                    egui::Event::Key { modifiers, .. } => Some(*modifiers),
                    _ => None,
                })
                .unwrap_or_default();
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 480.0))),
                    events,
                    modifiers,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let response = ui.add(egui::Button::new(if self.audio {
                            "Audio item"
                        } else {
                            "Video item"
                        }));
                        let keyboard = response.has_focus()
                            && ui.input(|i| i.modifiers.shift && i.key_pressed(egui::Key::F10));
                        if response.secondary_clicked() || keyboard {
                            select_menu_target(
                                &self.clip,
                                self.audio,
                                &mut self.playhead,
                                &mut self.library_preview,
                                &mut self.selected,
                                &mut self.selected_audio,
                            );
                        }
                        let frame = 1.0 / 30.0;
                        let enabled = [
                            can_split_at(&self.clip, frame, self.playhead),
                            can_trim_at(&self.clip, frame, self.playhead, self.audio, true),
                            can_trim_at(&self.clip, frame, self.playhead, self.audio, false),
                        ];
                        self.controls.clear();
                        context_menu(&response, keyboard, &mut self.keyboard_menu).show(|ui| {
                            if let Some(action) = edit_controls(ui, enabled, &mut self.controls) {
                                self.actions.push(action);
                            }
                        });
                        self.response = Some(response);
                    });
                },
            )
        }
        fn pointer(&mut self, pos: Pos2, button: egui::PointerButton, pressed: bool) {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: Default::default(),
                },
            ]);
        }
        fn open(&mut self, keyboard: bool) {
            self.frame(Vec::new());
            let response = self.response.as_ref().unwrap();
            if keyboard {
                self.ctx.memory_mut(|m| m.request_focus(response.id));
                self.frame(vec![egui::Event::Key {
                    key: egui::Key::F10,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers {
                        shift: true,
                        ..Default::default()
                    },
                }]);
            } else {
                let pos = response.rect.center();
                self.pointer(pos, egui::PointerButton::Secondary, true);
                self.pointer(pos, egui::PointerButton::Secondary, false);
            }
            self.frame(Vec::new()); // Let the popup finish its first sizing pass.
            assert_eq!(self.controls.len(), 3);
        }
    }

    #[test]
    fn injected_context_menus_preserve_playhead_and_dispatch_actions() {
        for audio in [false, true] {
            for keyboard in [false, true] {
                for (control, expected) in [
                    ("clip-menu-split", ClipAction::Split),
                    ("clip-menu-trim-before", ClipAction::TrimBefore),
                    ("clip-menu-trim-after", ClipAction::TrimAfter),
                ] {
                    let mut menu = MenuHarness::new(audio);
                    // Shift+F10 targets the focused selection; secondary click
                    // must select an otherwise unselected item without seeking.
                    if keyboard {
                        menu.selected = Some(menu.clip.id);
                        menu.selected_audio = audio.then_some(menu.clip.id);
                    }
                    menu.open(keyboard);
                    assert_eq!(
                        menu.playhead.to_bits(),
                        8.0_f64.to_bits(),
                        "Opening an item's menu moved the playhead"
                    );
                    assert!(!menu.library_preview);
                    assert_eq!(menu.selected_audio, audio.then_some(menu.clip.id));
                    if !audio {
                        assert_eq!(menu.selected, Some(menu.clip.id));
                    }
                    let settled = menu.controls.clone();
                    menu.frame(vec![egui::Event::PointerMoved(Pos2::new(550.0, 350.0))]);
                    assert_eq!(menu.controls, settled, "Menu moved after the opening frame");
                    let pos = menu.controls[control].center();
                    menu.pointer(pos, egui::PointerButton::Primary, true);
                    menu.pointer(pos, egui::PointerButton::Primary, false);
                    assert_eq!(menu.actions, [expected]);
                    assert!(!egui::Popup::is_any_open(&menu.ctx));
                }
            }
        }
    }

    #[test]
    fn injected_context_menu_disables_edits_outside_target_and_escape_closes_it() {
        for audio in [false, true] {
            let mut menu = MenuHarness::new(audio);
            menu.playhead = 6.0; // An existing timeline gap, outside the target.
            menu.open(false);
            let output = menu.frame(Vec::new());
            let tree = output.platform_output.accesskit_update.unwrap();
            for label in ["Split S", "Trim before Q", "Trim after W"] {
                let node = tree
                    .nodes
                    .iter()
                    .find_map(|(_, node)| {
                        (node.role() == egui::accesskit::Role::Button
                            && node.label() == Some(label))
                        .then_some(node)
                    })
                    .expect("Missing accessible menu action");
                assert!(node.is_disabled());
            }
            let pos = menu.controls["clip-menu-trim-before"].center();
            menu.pointer(pos, egui::PointerButton::Primary, true);
            menu.pointer(pos, egui::PointerButton::Primary, false);
            assert!(menu.actions.is_empty());
            assert_eq!(menu.playhead, 6.0);
            menu.frame(vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }]);
            assert!(!egui::Popup::is_any_open(&menu.ctx));
        }
    }
}
