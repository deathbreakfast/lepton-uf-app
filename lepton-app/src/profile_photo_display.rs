use leptos::prelude::*;
use meson_leptos::MesonImg;
use uf_product::primitives::{Avatar, AvatarConfig};

#[component]
pub fn ProfilePhotoDisplay(
    #[prop(into)] photo_file_id: MaybeProp<String>,
    #[prop(into)] display_name: Signal<String>,
) -> impl IntoView {
    view! {
        <div data-testid="profile-photo-display">
            {move || match photo_file_id.get() {
                Some(file_id) => {
                    view! {
                        <MesonImg
                            file_id=file_id
                            alt=display_name.get()
                            width="96px"
                            height="96px"
                            shape=orbital_primitives::ImageShape::Circular
                        />
                    }
                        .into_any()
                }
                None => {
                    view! {
                        <Avatar config=AvatarConfig {
                            src: None,
                            name: Some(display_name.get()),
                            size: Some(96),
                            ..Default::default()
                        } />
                    }
                        .into_any()
                }
            }}
        </div>
    }
}
