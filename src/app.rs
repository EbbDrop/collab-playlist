use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
};

use chrono::{TimeDelta, Utc};
use futures::{future::join_all, stream::TryStreamExt};
use leptos::{
    component, create_local_resource, expect_context, view, For, IntoView, Memo, SignalGet,
    SignalGetUntracked, SignalWith, Suspense,
};
use leptos_router::{use_params_map, Outlet};
use random_color::RandomColor;
use rgb::RGB8;
use rspotify::{
    clients::{BaseClient, OAuthClient},
    model::{PlayableItem, PlaylistId},
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
        <div class="selection">
            <h1>Your playlists:</h1>
            <Suspense fallback=|| view! { <h1>Loading</h1> }>
                <div class="selection-buttons">
                    <For
                        each=move || playlists().unwrap_or_default()
                        key=|playlist| playlist.id.clone()
                        let:playlist
                    >
                        <a
                            href=format!("/{}", Borrow::<str>::borrow(&playlist.id))
                            class="selection-button"
                        >
                            {playlist.name.clone()}
                            ": "
                            {if playlist.collaborative { "collaborative" } else { "solo" }}

                        </a>
                    </For>
                </div>
            </Suspense>
        </div>
        <Outlet/>
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TrackInfo {
    name: String,
    duration: TimeDelta,
    relative_size: f64,
    color: RGB8,
    age: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct UserInfo {
    name: String,
    relative_size: f64,
    total_duration: TimeDelta,
    amount_of_tracks: u64,
    color: RGB8,
}

#[derive(Debug, Clone, PartialEq)]
struct PlaylistInfo {
    name: String,
    total_duration: TimeDelta,

    tracks: Vec<TrackInfo>,
    users: Vec<UserInfo>,
}

fn display_duration(dur: &TimeDelta) -> String {
    let minutes = dur.num_minutes();
    let seconds = dur.num_seconds() - minutes * 60;
    format!("{minutes}:{seconds}")
}

#[component]
pub fn Playlist() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.with(|params| params.get("id").cloned().unwrap_or_default());

    let spotify = expect_context::<Memo<AuthCodePkceSpotify>>();

    let raw_data = create_local_resource(id, move |id| async move {
        let spotify = spotify.get_untracked();

        let id = PlaylistId::from_id(id).unwrap();

        let playlist = spotify.playlist(id, None, None).await.unwrap();

        let mut users = HashSet::new();

        for t in &playlist.tracks.items {
            if let Some(added_by) = &t.added_by {
                users.insert(added_by.id.clone());
            }
        }

        let user_names = join_all(users.into_iter().map(|id| (id, spotify.clone())).map(
            |(user_id, spotify)| async move {
                let Ok(user) = spotify.user(user_id.clone()).await else {
                    return (user_id, "Faild to get user".to_owned());
                };
                let name = user.display_name.unwrap_or_else(|| user.id.to_string());
                (user_id, name)
            },
        ))
        .await
        .into_iter()
        .collect::<HashMap<_, _>>();
        (playlist, user_names)
    });

    let data = move || {
        let Some((playlist, user_names)) = raw_data.get() else {
            return None;
        };

        let name = playlist.name;

        let mut total_duration = TimeDelta::default();
        let mut user_id_to_track = HashMap::new();

        for item in playlist.tracks.items {
            match item.track {
                Some(PlayableItem::Track(track)) => {
                    total_duration += track.duration;
                    user_id_to_track
                        .entry(item.added_by.map(|u| u.id))
                        .or_insert_with(Vec::new)
                        .push((item.added_at, track));
                }
                _ => {}
            }
        }

        let now = Utc::now();
        let mut data = user_id_to_track
            .into_iter()
            .map(|(user_id, groups)| {
                let color = RandomColor::new()
                    .seed(
                        user_id
                            .as_ref()
                            .map(|id| Borrow::<str>::borrow(id))
                            .unwrap_or_default(),
                    )
                    .to_rgb_array();
                let color: RGB8 = color.into();

                let mut user_tracks = groups
                    .into_iter()
                    .map(|(added_at, track)| {
                        let age = now.clone().signed_duration_since(added_at.unwrap_or(now));
                        let age = (age.num_days() as f64 / 200.0).clamp(0.0, 1.0);

                        TrackInfo {
                            name: track.name,
                            duration: track.duration,
                            relative_size: track.duration.num_milliseconds() as f64
                                / total_duration.num_milliseconds() as f64,
                            color: color.clone(),
                            age,
                        }
                    })
                    .collect::<Vec<_>>();

                user_tracks.sort_unstable_by(|a, b| a.duration.cmp(&b.duration));

                let user_name = user_id
                    .and_then(|id| user_names.get(&id).cloned())
                    .unwrap_or_else(|| "Unknow".to_owned());

                let user_total_duration: TimeDelta = user_tracks.iter().map(|t| &t.duration).sum();

                let user = UserInfo {
                    name: user_name,
                    relative_size: user_total_duration.num_milliseconds() as f64
                        / total_duration.num_milliseconds() as f64,
                    total_duration: user_total_duration,
                    amount_of_tracks: user_tracks.len() as u64,
                    color,
                };
                (user, user_tracks)
            })
            .collect::<Vec<_>>();

        data.sort_unstable_by(|a, b| a.0.total_duration.cmp(&b.0.total_duration));

        let mut tracks = Vec::new();
        let mut users = Vec::new();
        for (user, mut user_tracks) in data {
            tracks.append(&mut user_tracks);
            users.push(user);
        }

        Some(PlaylistInfo {
            name,
            total_duration,
            tracks,
            users,
        })
    };

    view! {
        <Suspense fallback=|| {
            view! { <h2>Loading playlist</h2> }
        }>
            {move || {
                data()
                    .map(|playlist| {
                        view! {
                            <h2>{format!("Playlist: \"{}\":", playlist.name)}</h2>
                            <table class="ribon-table">
                                <colgroup>
                                    {playlist
                                        .tracks
                                        .iter()
                                        .map(|track| {
                                            let width = format!("{}%", track.relative_size * 100.0);
                                            view! { <col style:width=width/> }
                                        })
                                        .collect::<Vec<_>>()}
                                </colgroup>
                                <tr>
                                    {playlist
                                        .users
                                        .into_iter()
                                        .map(|user| {
                                            let color = user.color.to_string();
                                            view! {
                                                <th
                                                    style=("--color", color)
                                                    colspan=user.amount_of_tracks.to_string()
                                                >
                                                    <div class="ribon-user-cell">
                                                        <span class="ribon-user-name">{user.name}</span>
                                                        <span class="ribon-user-time">
                                                            {format!(
                                                                "{} ({:.1}%)",
                                                                display_duration(&user.total_duration),
                                                                user.relative_size * 100.0,
                                                            )}

                                                        </span>
                                                    </div>
                                                </th>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                </tr>
                                <tr class="ribon-track-row">
                                    {playlist
                                        .tracks
                                        .iter()
                                        .map(|track| {
                                            let color = track.color.to_string();
                                            let age = format!("{}%", track.age / 2.0 * 100.0);
                                            view! {
                                                <th
                                                    style=("--color", color)
                                                    style=("--age", age)
                                                    class="ribon-track-cell"
                                                    title=track.name.clone()
                                                >
                                                    {if track.age > 0.99 {
                                                        Some(
                                                            view! {
                                                                <img
                                                                    class="ribon-track-cobweb ribon-track-cobweb-top"
                                                                    src="/cobweb-top.png"
                                                                />
                                                            },
                                                        )
                                                    } else {
                                                        None
                                                    }}
                                                    <div class="ribon-track-name">{track.name.clone()}</div>
                                                    {if track.age > 0.99 {
                                                        Some(
                                                            view! {
                                                                <img class="ribon-track-cobweb" src="/cobweb.png"/>
                                                            },
                                                        )
                                                    } else {
                                                        None
                                                    }}
                                                </th>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                </tr>
                            </table>
                        }
                    })
            }}

        </Suspense>
        <Outlet/>
    }
}
