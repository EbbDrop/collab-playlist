use futures::stream::TryStreamExt;
use leptos::{
    component, create_resource, expect_context, view, For, IntoView, Memo, SignalGetUntracked,
    Suspense,
};
use rspotify::{clients::OAuthClient, AuthCodePkceSpotify};

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
                    {playlist.name.clone()} ": "
                    {if playlist.collaborative { "collaborative" } else { "solo" }}

                </p>
            </For>
        </Suspense>
    }
}
