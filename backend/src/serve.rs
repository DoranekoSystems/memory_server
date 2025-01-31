use include_dir::{include_dir, Dir};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use warp::http::Response;
use warp::path::Tail;
use warp::Filter;

use crate::api;
use crate::logger;
use crate::native_bridge;
use crate::request;

pub async fn serve(mode: i32, host: IpAddr, port: u16) {
    let pid_state = Arc::new(Mutex::new(None));

    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["*", "Content-Type"])
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"]);

    let static_files = warp::path::tail()
        .map(|tail: Tail| tail.as_str().to_string())
        .and_then(serve_static);

    // Process and Module Routes
    let enum_process = warp::path!("processes")
        .and(warp::get())
        .and_then(api::enumerate_process_handler);

    let enum_module = warp::path!("modules")
        .and(warp::get())
        .and(api::with_state(pid_state.clone()))
        .and_then(|pid_state| async move { api::enummodule_handler(pid_state).await });

    let open_process = warp::path!("process")
        .and(warp::post())
        .and(warp::body::json())
        .and(api::with_state(pid_state.clone()))
        .and_then(|open_process, pid_state| async move {
            api::open_process_handler(pid_state, open_process).await
        });

    let change_process_state = warp::path!("process")
        .and(warp::put())
        .and(warp::body::json())
        .and(api::with_state(pid_state.clone()))
        .and_then(|state_request, pid_state| async move {
            api::change_process_state_handler(pid_state, state_request).await
        });

    // Memory Operation Routes
    let read_memory = warp::path!("memory")
        .and(warp::get())
        .and(warp::query::<request::ReadMemoryRequest>())
        .and(api::with_state(pid_state.clone()))
        .and_then(|read_memory_request, pid_state| async move {
            api::read_memory_handler(pid_state, read_memory_request).await
        });

    let write_memory = warp::path!("memory")
        .and(warp::post())
        .and(warp::body::json())
        .and(api::with_state(pid_state.clone()))
        .and_then(|write_memory, pid_state| async move {
            api::write_memory_handler(pid_state, write_memory).await
        });

    let read_memory_multiple = warp::path!("memories")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 1024 * 10)) // 10MB
        .and(warp::body::json::<Vec<request::ReadMemoryRequest>>())
        .and(api::with_state(pid_state.clone()))
        .and_then(|read_memory_requests, pid_state| async move {
            api::read_memory_multiple_handler(pid_state, read_memory_requests).await
        });

    // Memory Analysis Routes
    let memory_scan = warp::path!("memoryscan")
        .and(warp::post())
        .and(warp::body::json())
        .and(api::with_state(pid_state.clone()))
        .and_then(|scan_request, pid_state| async move {
            api::memory_scan_handler(pid_state, scan_request).await
        });

    let memory_filter = warp::path!("memoryfilter")
        .and(warp::post())
        .and(warp::body::json())
        .and(api::with_state(pid_state.clone()))
        .and_then(|filter_request, pid_state| async move {
            api::memory_filter_handler(pid_state, filter_request).await
        });

    let enum_regions = warp::path!("regions")
        .and(warp::get())
        .and(api::with_state(pid_state.clone()))
        .and_then(|pid_state| async move { api::enumerate_regions_handler(pid_state).await });

    // Debug Routes
    let set_watchpoint = warp::path!("watchpoint")
        .and(warp::post())
        .and(warp::body::json())
        .and(api::with_state(pid_state.clone()))
        .and_then(|set_watchpoint_request, pid_state| async move {
            api::set_watchpoint_handler(pid_state, set_watchpoint_request).await
        });

    let remove_watchpoint = warp::path!("watchpoint")
        .and(warp::delete())
        .and(warp::body::json())
        .and(api::with_state(pid_state.clone()))
        .and_then(|remove_watchpoint_request, pid_state| async move {
            api::remove_watchpoint_handler(pid_state, remove_watchpoint_request).await
        });

    let set_breakpoint = warp::path!("breakpoint")
        .and(warp::post())
        .and(warp::body::json())
        .and(api::with_state(pid_state.clone()))
        .and_then(|set_breakpoint_request, pid_state| async move {
            api::set_breakpoint_handler(pid_state, set_breakpoint_request).await
        });

    let remove_breakpoint = warp::path!("breakpoint")
        .and(warp::delete())
        .and(warp::body::json())
        .and(api::with_state(pid_state.clone()))
        .and_then(|remove_breakpoint_request, pid_state| async move {
            api::remove_breakpoint_handler(pid_state, remove_breakpoint_request).await
        });

    // Utility Routes
    let resolve_addr = warp::path!("resolveaddr")
        .and(warp::get())
        .and(warp::query::<request::ResolveAddrRequest>())
        .and(api::with_state(pid_state.clone()))
        .and_then(|resolve_addr_request, pid_state| async move {
            api::resolve_addr_handler(pid_state, resolve_addr_request).await
        });

    let explore_directory = warp::path!("directory")
        .and(warp::get())
        .and(warp::query::<request::ExploreDirectoryRequest>())
        .and_then(|explore_directory_request| async move {
            api::explore_directory_handler(explore_directory_request).await
        });

    let read_file = warp::path!("file")
        .and(warp::get())
        .and(warp::query::<request::ReadFileRequest>())
        .and_then(
            |read_file_request| async move { api::read_file_handler(read_file_request).await },
        );

    // Info Routes
    let get_app_info = warp::path!("appinfo")
        .and(warp::get())
        .and(api::with_state(pid_state.clone()))
        .and_then(|pid_state| async move { api::get_app_info_handler(pid_state).await });

    let server_info = warp::path!("serverinfo")
        .and(warp::get())
        .and_then(api::server_info_handler);

    let get_exception_info = warp::path!("exceptioninfo")
        .and(warp::get())
        .and_then(api::get_exception_info_handler);

    let pointermap_generate = warp::path!("pointermap")
        .and(warp::post())
        .and(warp::body::json())
        .and(api::with_state(pid_state.clone()))
        .and_then(|request, pid_state| async move {
            api::pointermap_generate_handler(pid_state, request).await
        });

    // Group routes by functionality
    let process_routes = enum_process
        .or(enum_module)
        .or(open_process)
        .or(change_process_state);

    let memory_operation_routes = read_memory.or(write_memory).or(read_memory_multiple);

    let memory_analysis_routes = memory_scan.or(memory_filter).or(enum_regions);

    let debug_routes = set_watchpoint
        .or(remove_watchpoint)
        .or(set_breakpoint)
        .or(remove_breakpoint);

    let utility_routes = resolve_addr.or(explore_directory).or(read_file);

    let info_routes = get_app_info
        .or(server_info)
        .or(get_exception_info)
        .or(pointermap_generate);

    // Combine all route groups
    let routes = process_routes
        .or(memory_operation_routes)
        .or(memory_analysis_routes)
        .or(debug_routes)
        .or(utility_routes)
        .or(info_routes)
        .or(static_files)
        .with(cors)
        .with(warp::log::custom(logger::http_log));

    native_bridge::native_api_init(mode);
    warp::serve(routes).run((host, port)).await;
}

static STATIC_DIR: Dir = include_dir!("../frontend/out");

async fn serve_static(path: String) -> Result<impl warp::Reply, warp::Rejection> {
    // Adjustment for include_dir! in windows environment
    let path = {
        #[cfg(host_os = "windows")]
        {
            path.replace("/", "\\")
        }
        #[cfg(not(host_os = "windows"))]
        {
            path
        }
    };
    match STATIC_DIR.get_file(&path) {
        Some(file) => {
            let mime_type = mime_guess::from_path(&path).first_or_octet_stream();
            Ok(Response::builder()
                .header("content-type", mime_type.as_ref())
                .body(file.contents()))
        }
        None => Err(warp::reject::not_found()),
    }
}
