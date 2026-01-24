//! Windows Service entry point for RuVector MemOpt

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use windows_service::{
        define_windows_service,
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode,
            ServiceState, ServiceStatus, ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };
    use std::ffi::OsString;
    use std::sync::mpsc;
    use std::time::Duration;
    
    const SERVICE_NAME: &str = "RuVectorMemOpt";
    
    define_windows_service!(ffi_service_main, service_main);
    
    fn service_main(arguments: Vec<OsString>) {
        if let Err(e) = run_service(arguments) {
            eprintln!("Service error: {}", e);
        }
    }
    
    fn run_service(_arguments: Vec<OsString>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        
        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop => {
                    let _ = shutdown_tx.send(());
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };
        
        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;
        
        // Report running
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;
        
        // Main service loop
        loop {
            match shutdown_rx.recv_timeout(Duration::from_secs(60)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Run optimization check here
                }
            }
        }
        
        // Report stopped
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;
        
        Ok(())
    }
    
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows service only runs on Windows");
}
