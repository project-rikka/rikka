use crate::support::{applied, clip, film, imported, inserted, square};
use cut_project::{
    Clip, Edge, Edit, EditPoint, Error, FrozenFrames, Item, Project, SceneItem, Switch, Transition,
    TransitionAlignment, TransitionKind, Viewpoint,
};

fn dissolve(project: &Project) -> Transition {
    Transition::new(TransitionKind::CrossDissolve, project.rate())
}

fn transited(project: &Project, edit_points: Vec<EditPoint>, transition: Transition) -> Project {
    applied(
        project,
        Edit::Transitions {
            edit_points,
            transition: Some(transition),
        },
    )
}

fn span(project: &Project, edit_point: EditPoint) -> Option<std::ops::Range<i64>> {
    project
        .transition_span(edit_point)
        .map(|transition_span| transition_span.span)
}

#[test]
fn transition_spans_both_clips_at_cut() {
    let (project, ids) = imported(vec![film("a", 4), film("b", 4)]);
    let transition = dissolve(&project);
    assert_eq!(transition.length(), 18);

    let project = transited(&project, vec![EditPoint::tail(ids[0])], transition);

    let transition_span = project
        .transition_span(EditPoint::tail(ids[0]))
        .expect("a passage");
    assert_eq!(transition_span.edit_point, EditPoint::head(ids[1]));
    assert_eq!(transition_span.out.map(Clip::id), Some(ids[0]));
    assert_eq!(transition_span.into.map(Clip::id), Some(ids[1]));
    assert_eq!(transition_span.cut, 100);
    assert_eq!(transition_span.span, 91..109);
    assert_eq!(
        project
            .transition_span_at(0, 95)
            .map(|transition_span| transition_span.edit_point),
        Some(EditPoint::head(ids[1]))
    );
    assert!(project.transition_span_at(0, 120).is_none());
    assert!(clip(&project, ids[0]).transitions().is_empty());
}

#[test]
fn alignment_places_span_around_cut() {
    let (project, ids) = imported(vec![film("a", 4), film("b", 4)]);
    let cases = [
        (TransitionAlignment::End, 82..100),
        (TransitionAlignment::Centre, 91..109),
        (TransitionAlignment::Start, 100..118),
    ];
    for (at, expected) in cases {
        let aligned = transited(
            &project,
            vec![EditPoint::head(ids[1])],
            dissolve(&project).with_alignment(at),
        );
        assert_eq!(
            span(&aligned, EditPoint::head(ids[1])),
            Some(expected),
            "{at:?}"
        );
        assert_eq!(
            aligned
                .transition_span(EditPoint::head(ids[1]))
                .and_then(|transition_span| transition_span.transition.alignment_at_cut()),
            Some(at)
        );
    }
}

#[test]
fn edge_without_neighbour_transitions_to_nothing() {
    let (project, ids) = imported(vec![film("a", 4)]);
    let project = transited(
        &project,
        vec![EditPoint::head(ids[0]), EditPoint::tail(ids[0])],
        dissolve(&project),
    );

    let opening = project
        .transition_span(EditPoint::head(ids[0]))
        .expect("an opening");
    assert_eq!((opening.out.is_none(), opening.span), (true, 0..18));
    let closing = project
        .transition_span(EditPoint::tail(ids[0]))
        .expect("a closing");
    assert_eq!((closing.into.is_none(), closing.span), (true, 82..100));
}

#[test]
fn long_transitions_share_clip_length() {
    let project = Project::new();
    let (project, first) = inserted(&project, square(10.0), 0, 10);
    let (project, second) = inserted(&project, square(10.0), 10, 10);
    let long = dissolve(&project).with_length(40);
    let project = transited(
        &project,
        vec![EditPoint::head(second), EditPoint::tail(second)],
        long,
    );

    let joined = span(&project, EditPoint::head(second)).expect("the joint");
    let closing = span(&project, EditPoint::tail(second)).expect("the closing");
    assert_eq!(joined.start, 0);
    assert!(joined.end <= closing.start, "{joined:?} {closing:?}");
    assert_eq!(closing.end, 20);
    assert_eq!(
        project
            .transition_span(EditPoint::tail(first))
            .map(|transition_span| transition_span.edit_point),
        Some(EditPoint::head(second))
    );
}

#[test]
fn moved_away_clip_leaves_open_transition() {
    let (project, ids) = imported(vec![film("a", 4), film("b", 4)]);
    let project = transited(&project, vec![EditPoint::head(ids[1])], dissolve(&project));

    let moved = applied(
        &project,
        Edit::Move {
            clips: vec![ids[1]],
            by: 50,
            across: 0,
        },
    );

    let transition_span = moved
        .transition_span(EditPoint::head(ids[1]))
        .expect("a passage");
    assert!(transition_span.out.is_none());
    assert_eq!(transition_span.span, 150..168);
}

#[test]
fn tail_transition_joins_following_clip() {
    let (project, ids) = imported(vec![film("a", 4)]);
    let project = transited(&project, vec![EditPoint::tail(ids[0])], dissolve(&project));
    let (project, applied) = project
        .apply(&Edit::Place {
            media: vec![film("b", 4)],
            at: 100,
            layer: 0,
        })
        .expect("placing");
    let second = applied.selected.expect("the placed clip")[0];

    assert_eq!(
        clip(&project, second).transitions().at(Edge::Head),
        Some(&dissolve(&project))
    );
    assert!(clip(&project, ids[0]).transitions().is_empty());
    assert_eq!(span(&project, EditPoint::head(second)), Some(91..109));
}

#[test]
fn split_adds_no_transition() {
    let (project, ids) = imported(vec![film("a", 4)]);
    let project = transited(
        &project,
        vec![EditPoint::head(ids[0]), EditPoint::tail(ids[0])],
        dissolve(&project),
    );

    let split = applied(
        &project,
        Edit::Split {
            clips: vec![ids[0]],
            at: 50,
        },
    );

    let transition_spans = split.transition_spans(0);
    assert_eq!(transition_spans.len(), 2);
    assert_eq!(transition_spans[0].span, 0..18);
    assert_eq!(transition_spans[1].span, 82..100);
}

#[test]
fn transition_applies_to_all_selected_joints() {
    let (project, ids) = imported(vec![film("a", 2), film("b", 2), film("c", 2), film("d", 2)]);

    let edit_points = project.edit_points_among(&ids[..3].iter().copied().collect());
    assert_eq!(
        edit_points,
        vec![EditPoint::head(ids[1]), EditPoint::head(ids[2])]
    );

    let project = transited(&project, edit_points, dissolve(&project));
    assert_eq!(project.transition_spans(0).len(), 2);
    let removed = applied(
        &project,
        Edit::Transitions {
            edit_points: vec![EditPoint::head(ids[1])],
            transition: None,
        },
    );
    assert_eq!(removed.transition_spans(0).len(), 1);
}

#[test]
fn transitions_reject_locked_layers_and_cameras() {
    let (project, ids) = imported(vec![film("a", 4)]);
    let refused = |project: &Project, edit_points: Vec<EditPoint>| {
        project
            .apply(&Edit::Transitions {
                edit_points,
                transition: Some(dissolve(project)),
            })
            .err()
    };

    assert!(matches!(
        refused(&project, Vec::new()),
        Some(Error::NoEditPoint)
    ));
    let (with_camera, camera) = inserted(
        &project,
        Item::Camera(cut_project::Camera::default()),
        0,
        20,
    );
    assert!(matches!(
        refused(&with_camera, vec![EditPoint::head(camera)]),
        Some(Error::NotVisual)
    ));
    let locked = applied(
        &project,
        Edit::Set {
            layer: 0,
            switch: Switch::Locked,
            on: true,
        },
    );
    assert!(matches!(
        refused(&locked, vec![EditPoint::head(ids[0])]),
        Some(Error::Locked)
    ));
}

#[test]
fn footage_without_handles_freezes_end_frames() {
    let (project, ids) = imported(vec![film("a", 4), film("b", 4)]);
    let project = transited(&project, vec![EditPoint::head(ids[1])], dissolve(&project));

    let transition_span = project
        .transition_span(EditPoint::head(ids[1]))
        .expect("a passage");
    assert_eq!(
        project.frozen_frames(&transition_span),
        FrozenFrames { out: 9, into: 9 }
    );

    let trimmed = applied(
        &project,
        Edit::Trim {
            clips: vec![ids[0]],
            edge: Edge::Tail,
            by: -20,
        },
    );
    let trimmed = applied(
        &trimmed,
        Edit::Move {
            clips: vec![ids[1]],
            by: -20,
            across: 0,
        },
    );
    let transition_span = trimmed
        .transition_span(EditPoint::head(ids[1]))
        .expect("a passage");
    assert_eq!(transition_span.cut, 80);
    assert_eq!(trimmed.frozen_frames(&transition_span).out, 0);
}

#[test]
fn audio_crossfade_uses_equal_power() {
    let (project, ids) = imported(vec![film("a", 4), film("b", 4)]);
    let project = applied(
        &project,
        Edit::Trim {
            clips: vec![ids[0]],
            edge: Edge::Tail,
            by: -20,
        },
    );
    let project = applied(
        &project,
        Edit::Trim {
            clips: vec![ids[1]],
            edge: Edge::Head,
            by: 20,
        },
    );
    let project = applied(
        &project,
        Edit::Move {
            clips: vec![ids[1]],
            by: -40,
            across: 0,
        },
    );
    let project = transited(&project, vec![EditPoint::head(ids[1])], dissolve(&project));

    let gains = |at: f64| -> Vec<(cut_project::ClipId, f32)> {
        project
            .voices_at(at)
            .iter()
            .map(|voice| (voice.clip.id(), voice.gain))
            .collect()
    };
    let middle = gains(80.0);
    assert_eq!(middle.len(), 2);
    let power: f32 = middle.iter().map(|(_, gain)| gain * gain).sum();
    assert!((power - 1.0).abs() < 1e-4, "{middle:?}");
    let early = gains(73.0);
    assert!(early[0].0 == ids[0] && early[0].1 > early[1].1, "{early:?}");
    assert_eq!(gains(40.0), vec![(ids[0], 1.0)]);
    assert_eq!(gains(120.0), vec![(ids[1], 1.0)]);
}

#[test]
fn continuous_source_keeps_one_voice() {
    let (project, ids) = imported(vec![film("a", 4)]);
    let project = applied(
        &project,
        Edit::Split {
            clips: vec![ids[0]],
            at: 50,
        },
    );
    let right = project.layers()[0].clips()[1].id();
    let project = transited(&project, vec![EditPoint::head(right)], dissolve(&project));

    let voices = project.voices_at(48.0);
    assert_eq!(voices.len(), 1);
    assert_eq!(voices[0].gain, 1.0);
}

#[test]
fn transition_span_shows_both_clips() {
    let (project, ids) = imported(vec![film("a", 4), film("b", 4)]);
    let project = transited(&project, vec![EditPoint::head(ids[1])], dissolve(&project));

    let scene = project.scene_restyled(95, Viewpoint::Output, |_| None);
    let [SceneItem::TransitionSpan { shots, .. }] = scene.as_slice() else {
        panic!("one passage, not {scene:?}");
    };
    assert!(shots.out.is_some() && shots.into.is_some());
    let later = project.scene_restyled(150, Viewpoint::Output, |_| None);
    assert!(matches!(later.as_slice(), [SceneItem::Clip { .. }]));
}
