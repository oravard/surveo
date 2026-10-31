//! Capture sous Wayland : portail ScreenCast (accord de l'utilisateur)
//! puis flux PipeWire (IMPL.md, 1.1).
//!
//! - Le jeton de restauration du portail est conservé : l'accord n'est
//!   demandé qu'une fois.
//! - Le flux est suspendu après chaque miniature et réactivé à la suivante :
//!   sans cela, un compositeur à cadence fixe (COSMIC) copie l'écran
//!   60 fois par seconde. La première image après le réveil est toujours
//!   prise.

use super::{CaptureEvent, CaptureOptions, Sink};
use crate::thumb::{compact, Order, Thumbnailer};
use anyhow::{anyhow, Context, Result};
use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};
use ashpd::desktop::PersistMode;
use pipewire as pw;
use pw::{properties::properties, spa};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn run(opts: &CaptureOptions, stop: &Arc<AtomicBool>, sink: &Sink) -> Result<()> {
    // Le portail parle D-Bus : ses tâches de fond tournent sur le fil du
    // moteur pendant que ce fil-ci fait tourner la boucle PipeWire.
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build()?;
    rt.block_on(async {
        let token = std::fs::read_to_string(&opts.token_file).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let proxy = Screencast::new().await.context("portail de capture d'écran indisponible")?;
        let session = proxy.create_session(Default::default()).await?;
        proxy
            .select_sources(
                &session,
                SelectSourcesOptions::default()
                    .set_cursor_mode(CursorMode::Embedded)
                    .set_sources(ashpd::enumflags2::BitFlags::from(SourceType::Monitor))
                    .set_multiple(false)
                    .set_restore_token(token.as_deref())
                    .set_persist_mode(PersistMode::ExplicitlyRevoked),
            )
            .await?;
        let response = proxy
            .start(&session, None, Default::default())
            .await?
            .response()
            .map_err(|_| anyhow!("partage de l'écran refusé sur le poste"))?;
        match response.restore_token() {
            Some(new) => {
                if let Some(dir) = opts.token_file.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(&opts.token_file, new);
            }
            None => log::info!("le portail n'a pas délivré de jeton : l'accord ne sera pas mémorisé"),
        }
        let stream = response.streams().first().ok_or_else(|| anyhow!("aucun écran partagé"))?.to_owned();
        let fd = proxy.open_pipe_wire_remote(&session, Default::default()).await?;
        let r = stream_frames(stream.pipe_wire_node_id(), fd, opts, stop, sink);
        let _ = session.close().await;
        r
    })
}

struct UserData {
    format: spa::param::video::VideoInfoRaw,
    thumbs: Thumbnailer,
    sink: Sink,
    warned: bool,
}

fn stream_frames(node_id: u32, fd: std::os::fd::OwnedFd, opts: &CaptureOptions, stop: &Arc<AtomicBool>, sink: &Sink) -> Result<()> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_fd_rc(fd, None)?;
    let failure: Rc<RefCell<Option<String>>> = Rc::default();

    let data = UserData { format: Default::default(), thumbs: Thumbnailer::new(opts.width, opts.quality), sink: sink.clone(), warned: false };

    let stream = pw::stream::StreamRc::new(
        core,
        "surveo-capture",
        properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )?;

    let (ml, fail) = (mainloop.clone(), failure.clone());
    let listener = stream
        .add_local_listener_with_user_data(data)
        .state_changed(move |_, _, _, new| {
            if let pw::stream::StreamState::Error(e) = new {
                *fail.borrow_mut() = Some(format!("flux d'écran interrompu : {e}"));
                ml.quit();
            }
        })
        .param_changed(|_, ud, id, param| {
            let Some(param) = param else { return };
            if id == pw::spa::param::ParamType::Format.as_raw() {
                let _ = ud.format.parse(param);
            }
        })
        .process(|stream, ud| {
            let Some(mut buffer) = stream.dequeue_buffer() else { return };
            let (w, h) = (ud.format.size().width, ud.format.size().height);
            let order = match ud.format.format() {
                spa::param::video::VideoFormat::BGRx | spa::param::video::VideoFormat::BGRA => Order::Bgra,
                spa::param::video::VideoFormat::RGBx | spa::param::video::VideoFormat::RGBA => Order::Rgba,
                other => {
                    if !ud.warned {
                        log::warn!("format de pixels non pris en charge : {other:?}");
                        ud.warned = true;
                    }
                    return;
                }
            };
            let datas = buffer.datas_mut();
            if datas.is_empty() || w == 0 {
                return;
            }
            let d = &mut datas[0];
            let (offset, size, stride) = (d.chunk().offset() as usize, d.chunk().size() as usize, d.chunk().stride() as usize);
            let Some(bytes) = d.data() else {
                if !ud.warned {
                    log::warn!("tampon d'écran non accessible en mémoire (DMA-BUF) : non pris en charge");
                    ud.warned = true;
                }
                return;
            };
            let end = offset + size.min(bytes.len() - offset);
            let frame = &mut bytes[offset..end];
            let mut owned;
            let pixels: &mut [u8] = if stride == w as usize * 4 {
                frame
            } else {
                owned = compact(frame, w, h, stride);
                &mut owned
            };
            match ud.thumbs.make(w, h, pixels, order) {
                Ok(jpeg) => {
                    (ud.sink)(CaptureEvent::Frame(jpeg));
                    let _ = stream.set_active(false);
                }
                Err(e) => log::warn!("miniature impossible : {e}"),
            }
        })
        .register()?;

    let obj = pw::spa::pod::object!(
        pw::spa::utils::SpaTypes::ObjectParamFormat,
        pw::spa::param::ParamType::EnumFormat,
        pw::spa::pod::property!(pw::spa::param::format::FormatProperties::MediaType, Id, pw::spa::param::format::MediaType::Video),
        pw::spa::pod::property!(pw::spa::param::format::FormatProperties::MediaSubtype, Id, pw::spa::param::format::MediaSubtype::Raw),
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            pw::spa::param::video::VideoFormat::BGRx,
            pw::spa::param::video::VideoFormat::BGRx,
            pw::spa::param::video::VideoFormat::BGRA,
            pw::spa::param::video::VideoFormat::RGBx,
            pw::spa::param::video::VideoFormat::RGBA,
        ),
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            pw::spa::utils::Rectangle { width: 1920, height: 1080 },
            pw::spa::utils::Rectangle { width: 1, height: 1 },
            pw::spa::utils::Rectangle { width: 16384, height: 16384 }
        ),
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::VideoFramerate,
            Choice,
            Range,
            Fraction,
            pw::spa::utils::Fraction { num: 1, denom: 1 },
            pw::spa::utils::Fraction { num: 0, denom: 1 },
            pw::spa::utils::Fraction { num: 60, denom: 1 }
        ),
    );
    let values: Vec<u8> =
        pw::spa::pod::serialize::PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &pw::spa::pod::Value::Object(obj))
            .map_err(|e| anyhow!("format PipeWire : {e:?}"))?
            .0
            .into_inner();
    let mut params = [spa::pod::Pod::from_bytes(&values).ok_or_else(|| anyhow!("format PipeWire invalide"))?];
    stream.connect(
        spa::utils::Direction::Input,
        Some(node_id),
        pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
        &mut params,
    )?;

    // À chaque intervalle : arrêt demandé, ou réveil du flux pour une image.
    let (ml, resume, stop) = (mainloop.clone(), stream.clone(), stop.clone());
    let timer = mainloop.loop_().add_timer(move |_| {
        if stop.load(Ordering::Relaxed) {
            ml.quit();
        } else {
            let _ = resume.set_active(true);
        }
    });
    timer.update_timer(Some(opts.interval), Some(opts.interval));
    mainloop.run();
    drop(timer);
    let _ = stream.disconnect();
    drop(listener);
    let failed = failure.borrow_mut().take();
    match failed {
        Some(e) => Err(anyhow!(e)),
        None => Ok(()),
    }
}
