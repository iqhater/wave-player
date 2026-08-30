extern crate embed_resource;

fn main() {
    embed_resource::compile("assets/app.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("failed to embed Windows icon");
}
