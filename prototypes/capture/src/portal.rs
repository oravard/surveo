//! Capture sous Wayland : portail ScreenCast (accord de l'utilisateur)
//! puis flux PipeWire. Le jeton de restauration du portail est conservé
//! pour que l'accord ne soit demandé qu'une fois (critère du prototype).

use crate::thumb::{compact, Order, Stats, Thumbnailer};
use crate::Args;
use anyhow::{anyhow, Context, Result};
use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};
use ashpd::desktop::PersistMode;
use pipewire as pw;
use pw::{properties::properties, spa};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

fn token_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("surveo-proto").join("jeton-capture.txt")
}

pub async fn run(args: &Args) -> Result<()> {
    let path = token_path();
    if args.forget {
        let _ = std::fs::remove_file(&path);
        println!("Jeton oublié : le portail redemandera votre accord.");
    }
    let token = std::fs::read_to_string(&path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

    let proxy = Screencast::new().await.context("portail ScreenCast indisponible")?;
    println!(
        "Portail ScreenCast version {} · jeton de restauration : {}",
        proxy.version(),
        if token.is_some() { "présent, fourni au portail" } else { "absent, le portail va demander votre accord" }
    );
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
    let t0 = Instant::now();
    let response = proxy.start(&session, None, Default::default()).await?.response().context("accord refusé ou annulé")?;
    println!("Accord obtenu en {} ms", t0.elapsed().as_millis());
    match response.restore_token() {
        Some(new) => {
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(&path, new)?;
            println!("Nouveau jeton reçu et enregistré dans {}", path.display());
        }
        None => println!("Le portail n'a délivré AUCUN jeton : l'accord ne sera pas mémorisé."),
    }
    let stream = response.streams().first().ok_or_else(|| anyhow!("aucun écran partagé"))?.to_owned();
    println!("Flux PipeWire n° {} · taille annoncée {:?}", stream.pipe_wire_node_id(), stream.size());
    let fd = proxy.open_pipe_wire_remote(&session, Default::default()).await?;

    // Référence : CPU du compositeur pendant 5 s, avant de brancher le flux.
    if let Some(c) = crate::thumb::Compositor::find() {
        let base = c.cpu_over(Duration::from_secs(5)).await;
        println!("Compositeur « {} » à vide : {:.1} % d'un cœur", c.name, base);
    }

    // La session du portail doit rester ouverte pendant la capture.
    let _keep = session;
    stream_frames(stream.pipe_wire_node_id(), fd, args)
}

struct UserData {
    format: spa::param::video::VideoInfoRaw,
    thumbs: Thumbnailer,
    stats: Rc<RefCell<Stats>>,
    interval: Duration,
    last: Option<Instant>,
    warned: bool,
    /// Suspendre le flux après chaque miniature (sinon le compositeur
    /// copie l'écran à sa cadence, 60 images/s sous COSMIC).
    pause: bool,
}

fn stream_frames(node_id: u32, fd: std::os::fd::OwnedFd, args: &Args) -> Result<()> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_fd_rc(fd, None)?;
    let stats = Rc::new(RefCell::new(Stats::new("wayland", args.out.clone())));

    let data = UserData {
        format: Default::default(),
        thumbs: Thumbnailer::new(args.width, args.quality),
        stats: stats.clone(),
        interval: Duration::from_millis(args.interval_ms),
        last: None,
        warned: false,
        pause: !args.continuous,
    };

    let stream = pw::stream::StreamRc::new(
        core,
        "surveo-capture",
        properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )?;

    let _listener = stream
        .add_local_listener_with_user_data(data)
        .param_changed(|_, ud, id, param| {
            let Some(param) = param else { return };
            if id != pw::spa::param::ParamType::Format.as_raw() {
                return;
            }
            if ud.format.parse(param).is_ok() {
                println!(
                    "Format négocié : {:?} {}×{} à {}/{} image/s",
                    ud.format.format(),
                    ud.format.size().width,
                    ud.format.size().height,
                    ud.format.framerate().num,
                    ud.format.framerate().denom
                );
            }
        })
        .process(|stream, ud| {
            let Some(mut buffer) = stream.dequeue_buffer() else { return };
            ud.stats.borrow_mut().seen();
            // Une miniature par intervalle : les autres images sont rendues aussitôt.
            // Flux suspendu : c'est la minuterie qui fixe la cadence, la première
            // image après le réveil est donc toujours prise.
            if !ud.pause && ud.last.is_some_and(|l| l.elapsed() < ud.interval) {
                return;
            }
            let (w, h) = (ud.format.size().width, ud.format.size().height);
            let order = match ud.format.format() {
                spa::param::video::VideoFormat::BGRx | spa::param::video::VideoFormat::BGRA => Order::Bgra,
                spa::param::video::VideoFormat::RGBx | spa::param::video::VideoFormat::RGBA => Order::Rgba,
                other => {
                    if !ud.warned {
                        println!("Format de pixels non traité par le prototype : {other:?}");
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
                    println!("Tampon non accessible en mémoire (DMA-BUF ?) : non traité par le prototype");
                    ud.warned = true;
                }
                return;
            };
            let t0 = Instant::now();
            let end = offset + size.min(bytes.len() - offset);
            let frame = &mut bytes[offset..end];
            let mut owned;
            let pixels: &mut [u8] = if stride == w as usize * 4 {
                frame
            } else {
                owned = compact(frame, w, h, stride);
                &mut owned
            };
            let capture_ms = t0.elapsed().as_secs_f64() * 1000.0;
            match ud.thumbs.make(w, h, pixels, order) {
                Ok(t) => {
                    ud.stats.borrow_mut().record(w, h, capture_ms, &t);
                    ud.last = Some(Instant::now());
                    if ud.pause {
                        let _ = stream.set_active(false);
                    }
                }
                Err(e) => println!("Miniature impossible : {e}"),
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
    let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(obj),
    )
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

    // Bilan toutes les 10 s, arrêt au bout de la durée demandée.
    let started = Instant::now();
    let (ml, st, duration) = (mainloop.clone(), stats.clone(), Duration::from_secs(args.duration));
    let (resume, pause) = (stream.clone(), !args.continuous);
    let ticks = Rc::new(RefCell::new(0u32));
    let timer = mainloop.loop_().add_timer(move |_| {
        let mut n = ticks.borrow_mut();
        *n += 1;
        if *n % 10 == 0 {
            st.borrow_mut().report();
        }
        // Réveil du flux : une nouvelle image sera livrée, puis le flux se suspend.
        if pause {
            let _ = resume.set_active(true);
        }
        if started.elapsed() >= duration {
            ml.quit();
        }
    });
    timer.update_timer(Some(Duration::from_secs(1)), Some(Duration::from_secs(1)));
    mainloop.run();
    stats.borrow_mut().summary();
    Ok(())
}
