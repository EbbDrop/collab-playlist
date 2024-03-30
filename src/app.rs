use std::borrow::Borrow;

use chrono::TimeDelta;
use futures::stream::TryStreamExt;
use leptos::{
    component, create_local_resource, expect_context, view, For, IntoView, Memo, SignalGet,
    SignalGetUntracked, SignalWith, Suspense,
};
use leptos_router::{use_params_map, Outlet};
use random_color::RandomColor;
use rspotify::{
    clients::{BaseClient, OAuthClient},
    model::{PlayableItem, PlaylistId, UserId},
    prelude::Id,
    AuthCodePkceSpotify,
};

#[component]
pub fn MainPage() -> impl IntoView {
    let spotify = expect_context::<Memo<AuthCodePkceSpotify>>();

    let playlists = create_local_resource(
        || (),
        move |_| async move {
            let spotify = spotify.get_untracked();
            let playlists_stream = spotify.current_user_playlists();

            let v: Vec<_> = playlists_stream.try_collect().await.unwrap();

            v
        },
    );

    view! {
        <Suspense fallback=|| view! { <h1>Loading</h1> }>
            <h1>Playlists:</h1>
            <For
                each=move || playlists().unwrap_or_default()
                key=|playlist| playlist.id.clone()
                let:playlist
            >
                <p>
                    <a href=format!(
                        "/{}",
                        Borrow::<str>::borrow(&playlist.id),
                    )>
                        {playlist.name.clone()} ": "
                        {if playlist.collaborative { "collaborative" } else { "solo" }}

                    </a>
                </p>
            </For>
        </Suspense>
        <Outlet/>
    }
}

#[component]
pub fn Playlist() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.with(|params| params.get("id").cloned().unwrap_or_default());

    let spotify = expect_context::<Memo<AuthCodePkceSpotify>>();

    let playlist = create_local_resource(id, move |id| async move {
        let spotify = spotify.get_untracked();

        let id = PlaylistId::from_id(id).unwrap();

        let mut playlist = spotify.playlist(id, None, None).await.unwrap();
        playlist.tracks.items.sort_by(|a, b| {
            a.added_by
                .as_ref()
                .map(|u| u.id.uri())
                .cmp(&b.added_by.as_ref().map(|u| u.id.uri()))
        });

        let mut total = TimeDelta::default();

        let mut people: Vec<(Option<UserId>, u64, TimeDelta)> = Vec::new();

        for pl_track in &playlist.tracks.items {
            match &pl_track.track {
                Some(PlayableItem::Track(track)) => {
                    let id = pl_track.added_by.as_ref().map(|u| u.id.clone());
                    if let Some(last) = people.last_mut() {
                        if last.0 == id {
                            last.1 += 1;
                            last.2 += track.duration;
                        } else {
                            people.push((id, 1, track.duration.clone()))
                        }
                    } else {
                        people.push((id, 1, track.duration.clone()))
                    }

                    total += track.duration;
                }
                _ => {}
            }
        }

        let people = people
            .into_iter()
            .map(|(id, amount, duration)| {
                let user = create_local_resource(
                    move || id.clone(),
                    move |id| async move {
                        let spotify = expect_context::<Memo<AuthCodePkceSpotify>>();
                        let Some(id) = id else {
                            return "Unknow".to_owned();
                        };
                        let user = spotify.get_untracked().user(id).await.unwrap();
                        user.display_name.unwrap_or_else(|| user.id.to_string())
                    },
                );
                (user, amount, duration)
            })
            .collect::<Vec<_>>();

        (playlist, total.num_milliseconds(), people)
    });

    view! {
        <Suspense fallback=|| {
            view! { <h2>Loading playlist</h2> }
        }>

            {move || {
                playlist()
                    .map(|(playlist, num_milliseconds, people)| {
                        view! {
                            <h2>"Playlist \"" {playlist.name.clone()} "\":"</h2>
                            <table style="width:100%;height:100%;table-layout:fixed;overflow:hidden">
                                <tbody>
                                    <tr>
                                        {playlist
                                            .tracks
                                            .items
                                            .iter()
                                            .filter_map(|track| {
                                                let color = RandomColor::new()
                                                    .seed::<
                                                        &str,
                                                    >(
                                                        track
                                                            .added_by
                                                            .as_ref()
                                                            .map(|user| user.id.borrow())
                                                            .unwrap_or_default(),
                                                    )
                                                    .to_hex();
                                                match &track.track {
                                                    Some(PlayableItem::Track(track)) => {
                                                        let name = track.name.clone();
                                                        let duration = track.duration.clone();
                                                        Some((name, duration, color))
                                                    }
                                                    _ => None,
                                                }
                                            })
                                            .map(|(name, duration, color)| {
                                                let width = duration.num_milliseconds() as f64
                                                    / num_milliseconds as f64;
                                                let width = format!("{width}%");
                                                view! {
                                                    <th
                                                        style:width=width
                                                        style:background=color
                                                        style:writing-mode="vertical-rl"
                                                    >
                                                        {name}
                                                    </th>
                                                }
                                            })
                                            .collect::<Vec<_>>()}
                                    </tr>
                                    <tr>
                                        {people
                                            .into_iter()
                                            .map(|(user, amount, duration)| {
                                                view! {
                                                    <th colspan={amount.to_string()}>
                                <p>{
                                    let minutes = duration.num_minutes();
                                    let seconds = duration.num_seconds() - minutes * 60;
                                    format!("{minutes}:{seconds} ({:.1}%)", duration.num_milliseconds() as f64 / num_milliseconds as f64 * 100.0)
                                }</p>
                                                    <Suspense fallback=|| {
                                                        view! { <p>Loading username</p> }
                                                    }><p>{user.get().map(|u| u.clone())}</p></Suspense>

                                                    </th>
                                                }
                                            })
                                            .collect::<Vec<_>>()}
                                    </tr>
                                </tbody>
                            </table>
                        }
                    })
            }}

        </Suspense>
        <Outlet/>
    }
}
