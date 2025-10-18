use eframe::UserEvent;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow},
    window::WindowId,
};

/// eframe 0.33.3 can leave Poll set after its last redraw. Preserve its timed waits,
/// but return to an OS wait when no deadline remains. This adapter owns no window state.
pub struct Reactive<T>(pub T);

impl<T: ApplicationHandler<UserEvent>> ApplicationHandler<UserEvent> for Reactive<T> {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        self.0.new_events(event_loop, cause);
    }
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.0.resumed(event_loop);
    }
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.0.suspended(event_loop);
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        self.0.user_event(event_loop, event);
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.0.window_event(event_loop, id, event);
    }
    fn device_event(&mut self, event_loop: &ActiveEventLoop, id: DeviceId, event: DeviceEvent) {
        self.0.device_event(event_loop, id, event);
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.0.about_to_wait(event_loop);
        if matches!(event_loop.control_flow(), ControlFlow::Poll) {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.0.exiting(event_loop);
    }
    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        self.0.memory_warning(event_loop);
    }
}
