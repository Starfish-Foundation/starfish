#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .setup(|_app| {
            // The desktop shell owns the local backend. If another BFish
            // process is already serving the API, the UI will simply use it.
            std::thread::spawn(|| {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(err) => {
                        eprintln!("BFish: could not create runtime: {err}");
                        return;
                    }
                };
                runtime.block_on(async {
                    if let Err(err) = bfish_backend::serve(8765).await {
                        eprintln!("BFish backend did not start: {err:#}");
                    }
                });
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running BFish");
}
