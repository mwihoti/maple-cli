use maplecode_protocol::models::DEFAULT_IMAGE_DETAIL;
use maplecode_protocol::models::FunctionCallOutputContentItem;
use maplecode_protocol::models::ImageReference;

trait IntoProtocol<T> {
    fn into_protocol(self) -> T;
}

pub(super) fn into_function_call_output_content_items(
    items: Vec<maplecode_code_mode::FunctionCallOutputContentItem>,
) -> Vec<FunctionCallOutputContentItem> {
    items.into_iter().map(IntoProtocol::into_protocol).collect()
}

impl IntoProtocol<FunctionCallOutputContentItem>
    for maplecode_code_mode::FunctionCallOutputContentItem
{
    fn into_protocol(self) -> FunctionCallOutputContentItem {
        let value = self;
        match value {
            maplecode_code_mode::FunctionCallOutputContentItem::InputText { text } => {
                FunctionCallOutputContentItem::InputText { text }
            }
            maplecode_code_mode::FunctionCallOutputContentItem::InputImage { image_url, detail } => {
                FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::Inline { image_url },
                    detail: detail.or(Some(DEFAULT_IMAGE_DETAIL)),
                }
            }
            maplecode_code_mode::FunctionCallOutputContentItem::InputAudio { audio_url } => {
                FunctionCallOutputContentItem::inline_audio(audio_url)
            }
        }
    }
}
