use std::borrow::Borrow;

use futures::stream::TryStreamExt;
use leptos::{
    component, create_resource, expect_context, view, For, IntoView, Memo, SignalGetUntracked,
    SignalWith, Suspense,
};
use leptos_router::{use_params_map, Outlet};
use rspotify::{
    clients::{BaseClient, OAuthClient},
    model::{PlayableItem, PlaylistId},
    AuthCodePkceSpotify,
};

#[component]
pub fn MainPage() -> impl IntoView {
    let spotify = expect_context::<Memo<AuthCodePkceSpotify>>();

    let playlists = create_resource(
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

    let playlists = create_resource(id, move |id| async move {
        let spotify = spotify.get_untracked();

        let id = PlaylistId::from_id(id).unwrap();

        spotify.playlist(id, None, None).await.unwrap()
    });

    view! {
        <Suspense fallback=|| {
            view! { <h2>Loading playlist</h2> }
        }>

            {move || {
                playlists()
                    .map(|playlist| {
                        view! {
                            <h2>"Playlist \"" {playlist.name.clone()} ":"</h2>

                            {playlist
                                .tracks
                                .items
                                .iter()
                                .map(|track| {
                                    view! {
                                        <p>
                                            {match &track.track {
                                                Some(PlayableItem::Track(track)) => {
                                                    let name = track.name.clone();
                                                    Some(name)
                                                }
                                                _ => None,
                                            }}

                                        </p>
                                    }
                                })
                                .collect::<Vec<_>>()}
                        }
                    })
            }}

        </Suspense>
        <Outlet/>
    }
}
