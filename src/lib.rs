mod ring_style;
mod rainbow_style;

use ring_style::Visualizer;
use rainbow_style::Bg;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioContext, AudioBufferSourceNode, AnalyserNode, HtmlCanvasElement, CanvasRenderingContext2d,
};
use js_sys::Uint8Array;
use wasm_bindgen::JsCast;
use futures::channel::oneshot;
use wasm_bindgen::closure::Closure;

#[wasm_bindgen]
#[derive(Clone, Copy, PartialEq)]
pub enum StyleType {
    Visualizer,
    Bg,
}

#[wasm_bindgen]
pub struct SharedAudioProcessor {
    context: AudioContext,
    analyser: AnalyserNode,
    source: Option<Rc<RefCell<AudioBufferSourceNode>>>,
    is_playing: bool,
    on_audio_end: Option<js_sys::Function>,
    instances: Rc<RefCell<Vec<AudioVisualizerInstance>>>,
}

#[wasm_bindgen]
impl SharedAudioProcessor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<SharedAudioProcessor, JsValue> {
        console_error_panic_hook::set_once();

        let context = AudioContext::new()?;
        let analyser = context.create_analyser()?;
        analyser.set_fft_size(256);
        analyser.set_smoothing_time_constant(0.8);

        Ok(SharedAudioProcessor {
            context,
            analyser,
            source: None,
            is_playing: false,
            on_audio_end: None,
            instances: Rc::new(RefCell::new(Vec::new())),
        })
    }

    #[wasm_bindgen]
    pub fn add_instance(
        &mut self,
        canvas: HtmlCanvasElement,
        style_type: StyleType,
    ) -> Result<usize, JsValue> {
        let instance = AudioVisualizerInstance::new(canvas, style_type)?;
        self.instances.borrow_mut().push(instance);
        Ok(self.instances.borrow().len() - 1)
    }

    #[wasm_bindgen]
    pub fn set_on_audio_end(&mut self, callback: js_sys::Function) {
        self.on_audio_end = Some(callback);
    }

    #[wasm_bindgen]
    pub async fn process_audio_from_path(&mut self, path: &str) -> Result<(), JsValue> {
        use web_sys::{MediaSource, Response, HtmlMediaElement};

        log("Starting streaming audio processing");

        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window found"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("No document found"))?;
        let audio_element: HtmlMediaElement = document.create_element("audio")?.dyn_into()?;

        let media_source = MediaSource::new()?;
        let media_url = web_sys::Url::create_object_url_with_source(&media_source)?;

        audio_element.set_src(&media_url);
        audio_element.set_cross_origin(Some("anonymous"));

        let media_element_source = self.context.create_media_element_source(&audio_element)?;
        media_element_source.connect_with_audio_node(&self.analyser)?;
        self.analyser
            .connect_with_audio_node(&self.context.destination())?;

        let media_source_clone = media_source.clone();
        let window_clone = window.clone();

        let server_url = if !path.starts_with("http") {
            format!("http://127.0.0.1:3000{}", path)
        } else {
            path.to_string()
        };

        let on_source_open = Closure::once(Box::new(move || {
            wasm_bindgen_futures::spawn_local(async move {
                match async move {
                    log("MediaSource opened, creating SourceBuffer");
                    let source_buffer = media_source_clone.add_source_buffer("audio/mpeg")?;

                    let fetch_promise = window_clone.fetch_with_str(&server_url);
                    let response: Response =
                        JsFuture::from(fetch_promise).await?.dyn_into()?;

                    if !response.ok() {
                        return Err(JsValue::from_str("Failed to fetch audio file"));
                    }

                    let body = response
                        .body()
                        .ok_or_else(|| JsValue::from_str("No response body"))?;
                    let reader = body
                        .get_reader()
                        .dyn_into::<web_sys::ReadableStreamDefaultReader>()?;

                    loop {
                        let chunk = JsFuture::from(reader.read()).await?;
                        let obj = js_sys::Object::from(chunk);

                        let done = js_sys::Reflect::get(&obj, &"done".into())?
                            .as_bool()
                            .unwrap_or(false);

                        if done {
                            log("All data has been read, ending stream");
                            media_source_clone.end_of_stream()?;
                            break;
                        }

                        if let Ok(value) = js_sys::Reflect::get(&obj, &"value".into()) {
                            let array = js_sys::Uint8Array::new(&value);

                            source_buffer.append_buffer_with_array_buffer(&array.buffer())?;
                            wait_for_updateend(&source_buffer).await?;
                            log("Successfully appended buffer");
                        }
                    }
                    Ok(())
                }
                .await
                {
                    Ok(()) => (),
                    Err(e) => {
                        web_sys::console::error_1(&e);
                    }
                }
            });
        }));
        media_source.set_onsourceopen(Some(on_source_open.as_ref().unchecked_ref()));
        on_source_open.forget();

        let on_ended = {
            let on_audio_end = self.on_audio_end.clone();
            Closure::wrap(Box::new(move || {
                log("Audio playback ended");
                if let Some(ref callback) = on_audio_end {
                    let this = JsValue::NULL;
                    let _ = callback.call0(&this);
                }
            }) as Box<dyn FnMut()>)
        };
        audio_element.set_onended(Some(on_ended.as_ref().unchecked_ref()));
        on_ended.forget();

        let play_promise = audio_element.play()?;
        JsFuture::from(play_promise).await?;

        self.is_playing = true;

        audio_element.set_attribute("style", "display: none")?;
        document
            .body()
            .ok_or_else(|| JsValue::from_str("No body found"))?
            .append_child(&audio_element)?;

        Ok(())
    }

    #[wasm_bindgen]
    pub fn stop_audio(&mut self) -> Result<(), JsValue> {
        self.is_playing = false;

        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                let audio_elements = document.get_elements_by_tag_name("audio");
                let length = audio_elements.length();
                for i in 0..length {
                    if let Some(audio) = audio_elements.item(i) {
                        if let Some(parent) = audio.parent_node() {
                            parent.remove_child(&audio)?;
                        }
                    }
                }
            }
        }

        self.clear_all();
        Ok(())
    }

    #[wasm_bindgen]
    pub fn draw(&self) {
        if !self.is_playing {
            return;
        }

        let buffer_length = self.analyser.frequency_bin_count();
        let mut data_array = vec![0u8; buffer_length as usize];
        self.analyser.get_byte_frequency_data(&mut data_array);

        let mut instances = self.instances.borrow_mut();
        for instance in instances.iter_mut() {
            instance.draw(&data_array);
        }
    }

    #[wasm_bindgen]
    pub fn clear_all(&self) {
        let mut instances = self.instances.borrow_mut();
        for instance in instances.iter_mut() {
            instance.clear_canvas();
        }
    }
}

struct AudioVisualizerInstance {
    visualizer: Option<Visualizer>,
    bg: Option<Bg>,
    style_type: StyleType,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
}

impl AudioVisualizerInstance {
    fn new(canvas: HtmlCanvasElement, style_type: StyleType) -> Result<Self, JsValue> {
        let ctx = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("Failed to get 2D context"))?
            .dyn_into::<CanvasRenderingContext2d>()?;

        let visualizer = if style_type == StyleType::Visualizer {
            Some(Visualizer::new(canvas.clone())?)
        } else {
            None
        };

        let bg = if style_type == StyleType::Bg {
            Some(Bg::new(canvas.clone())?)
        } else {
            None
        };

        Ok(AudioVisualizerInstance {
            visualizer,
            bg,
            style_type,
            canvas,
            ctx,
        })
    }

    fn draw(&mut self, audio_data: &[u8]) {
        match self.style_type {
            StyleType::Visualizer => {
                if let Some(ref mut visualizer) = self.visualizer {
                    visualizer.draw(audio_data);
                }
            }
            StyleType::Bg => {
                if let Some(ref mut bg) = self.bg {
                    bg.draw(audio_data);
                }
            }
        }
    }

    fn clear_canvas(&self) {
        self.ctx.clear_rect(
            0.0,
            0.0,
            self.canvas.width() as f64,
            self.canvas.height() as f64,
        );
    }
}

fn log(s: &str) {
    web_sys::console::log_1(&JsValue::from_str(s));
}

async fn wait_for_updateend(source_buffer: &web_sys::SourceBuffer) -> Result<(), JsValue> {
    use futures::channel::oneshot;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct UpdateEndHandler {
        closure: Closure<dyn FnMut()>,
    }

    impl UpdateEndHandler {
        fn new(source_buffer: &web_sys::SourceBuffer) -> (Rc<RefCell<Option<Self>>>, oneshot::Receiver<()>) {
            let (sender, receiver) = oneshot::channel::<()>();
            let handler = Rc::new(RefCell::new(None));

            let handler_clone = handler.clone();
            let sender = Rc::new(RefCell::new(Some(sender)));

            let closure = Closure::wrap(Box::new(move || {
                if let Some(sender) = sender.borrow_mut().take() {
                    let _ = sender.send(());
                }

                handler_clone.borrow_mut().take();
            }) as Box<dyn FnMut()>);

            source_buffer.set_onupdateend(Some(closure.as_ref().unchecked_ref()));

            *handler.borrow_mut() = Some(UpdateEndHandler { closure });

            (handler, receiver)
        }
    }

    let (_handler, receiver) = UpdateEndHandler::new(source_buffer);

    receiver.await.map_err(|_| JsValue::from_str("Failed to receive updateend event"))
}
