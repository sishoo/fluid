use ash::extensions::{
    ext::DebugUtils,
    khr::{Surface, Swapchain},
};
use ash::vk::DeviceCreateInfo;
use ash::{vk, Entry};
pub use ash::{Device, Instance};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::borrow::Cow;
use std::cell::RefCell;
use std::default::Default;
use std::ffi::CStr;
use std::fs::File;
use std::io::Read;
use std::ops::Drop;
use std::os::raw::c_char;

#[cfg(target_os = "macos")]
use ash::vk::{
    KhrGetPhysicalDeviceProperties2Fn, KhrPortabilityEnumerationFn, KhrPortabilitySubsetFn,
};

use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

pub fn compile_glsl_to_spirv(glsl_path: &str, shader_type: u8) -> Result<Vec<u32>, std::io::Error> {
    let mut compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.add_macro_definition("EP", Some("main"));

    let mut src_text = String::new();
    let _size = File::open(glsl_path)?.read_to_string(&mut src_text);

    let bin_res = if shader_type == 0 {
        compiler
            .compile_into_spirv(
                &src_text,
                shaderc::ShaderKind::Vertex,
                "vertex.vert",
                "main",
                Some(&options),
            )
            .unwrap()
            .as_binary()
            .to_vec()
    } else if shader_type == 1 {
        compiler
            .compile_into_spirv(
                &src_text,
                shaderc::ShaderKind::Fragment,
                "fragment.frag",
                "main",
                Some(&options),
            )
            .unwrap()
            .as_binary()
            .to_vec()
    } else {
        panic!("CMON BRUH");
    };
    Ok(bin_res)
}

pub fn record_submit_commandbuffer<F: FnOnce(&Device, vk::CommandBuffer)>(
    device: &Device,                       //  logical device
    command_buffer: vk::CommandBuffer,     // command buffer
    command_buffer_reuse_fence: vk::Fence, //  fence so that it knows when the command buffer can be reused
    submit_queue: vk::Queue,
    wait_semaphores: &[vk::Semaphore], // semaphores that the commands are waiting on
    signal_semaphores: &[vk::Semaphore], // semaphore that says when the command is done
    wait_mask: &[vk::PipelineStageFlags],
    record_commands: F, // closure that contains the commands to be recorded
) -> Result<(), vk::Result> {
    unsafe {
        // println!("REMINDER TO COME BACK TO FIX THIS ERROR HANDLING IN RECORD_COMMAND_BUFFER");

        // waits for a fence signaling that the command buffer is done being processed and is ready for reuse
        device
            .wait_for_fences(&[command_buffer_reuse_fence], true, std::u64::MAX)
            .expect("Wait for fence failed");

        // once the command buffer is done being used we need to reset it
        device
            .reset_fences(&[command_buffer_reuse_fence])
            .expect("Resetting the fences for reuse failed");

        // the command buffer is reset so we can reuse it
        device
            .reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )
            .expect("Failed to reset the command buffer");

        // before the recording can start we need an info struct
        let mut command_buffer_begin_info = vk::CommandBufferBeginInfo::default();
        command_buffer_begin_info.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT;

        // start the recording
        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .expect("Failed to begin the command buffer recording");

        // do the recording
        // a closure is passed in at the function call site that allows you to enter custom commands in what ever order you want
        record_commands(device, command_buffer);

        // end the recording
        device
            .end_command_buffer(command_buffer)
            .expect("Failed to end the command buffer recording");

        let command_buffers = vec![command_buffer];

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            // dst meaning destination
            .wait_dst_stage_mask(wait_mask)
            .command_buffers(&command_buffers)
            .signal_semaphores(signal_semaphores);

        device.queue_submit(
            submit_queue,
            &[submit_info.build()],
            command_buffer_reuse_fence,
        )
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    println!("{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n");

    vk::FALSE
}

pub fn find_memory_type_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    memory_prop.memory_types[..memory_prop.memory_type_count as _]
        .iter()
        .enumerate()
        .find(|(index, memory_type)| {
            (1 << index) & memory_req.memory_type_bits != 0
                && memory_type.property_flags & flags == flags
        })
        .map(|(index, _memory_type)| index as _)
}

pub struct Base {
    pub entry: Entry,
    pub instance: Instance,
    pub device: Device,
    pub surface_loader: Surface,
    pub swapchain_loader: Swapchain,
    pub debug_utils_loader: DebugUtils,
    pub window: winit::window::Window,
    pub event_loop: RefCell<EventLoop<()>>,
    pub debug_call_back: vk::DebugUtilsMessengerEXT,

    pub physical_device: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub queue_family_index: u32,
    pub command_submit_queue: vk::Queue,

    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub surface_resolution: vk::Extent2D,

    pub swapchain: vk::SwapchainKHR,
    pub present_images: Vec<vk::Image>,
    pub present_image_views: Vec<vk::ImageView>,

    pub command_pool: vk::CommandPool,
    pub draw_command_buffer: vk::CommandBuffer,
    pub setup_command_buffer: vk::CommandBuffer,

    pub depth_image: vk::Image,
    pub depth_image_view: vk::ImageView,
    pub depth_image_memory: vk::DeviceMemory,

    pub present_complete_semaphore: vk::Semaphore,
    pub rendering_complete_semaphore: vk::Semaphore,

    pub draw_command_buffer_reuse_fence: vk::Fence,
    pub setup_command_buffer_reuse_fence: vk::Fence,
}

impl Base {
    pub fn render_loop<F: Fn()>(&self, f: F) {
        self.event_loop
            .borrow_mut()
            .run_return(|event, _, control_flow| {
                *control_flow = ControlFlow::Poll;
                match event {
                    Event::WindowEvent {
                        event:
                            WindowEvent::CloseRequested
                            | WindowEvent::KeyboardInput {
                                input:
                                    KeyboardInput {
                                        state: ElementState::Pressed,
                                        virtual_keycode: Some(VirtualKeyCode::Escape),
                                        ..
                                    },
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    Event::MainEventsCleared => f(),
                    _ => (),
                }
            });
    }

    pub fn new(window_width: u32, window_height: u32, name: &str) -> Self {
        unsafe {
            let event_loop = EventLoop::new();
            let window = WindowBuilder::new()
                .with_title(name)
                .with_inner_size(winit::dpi::LogicalSize::new(window_width, window_height))
                .build(&event_loop)
                .unwrap();

            let entry = Entry::linked(); // links the functions from a vulkan loader
            let app_name = CStr::from_bytes_with_nul_unchecked(b"VulkanTriangle\0");

            let layer_names = [
                CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0"),
                // CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_LUNARG_monitor\0"),
            ];

            let layer_names_raw: Vec<*const c_char> =
                layer_names.iter().map(|name| name.as_ptr()).collect();

            let mut extension_names =
                ash_window::enumerate_required_extensions(window.raw_display_handle())
                    .unwrap()
                    .to_vec();
            extension_names.push(DebugUtils::name().as_ptr());

            #[cfg(target_os = "macos")]
            {
                extension_names.push(KhrPortabilityEnumerationFn::name().as_ptr());
                extension_names.push(KhrGetPhysicalDeviceProperties2Fn::name().as_ptr());
            }

            let app_info = vk::ApplicationInfo::builder()
                .application_name(app_name)
                .application_version(0)
                .engine_name(app_name)
                .engine_version(0)
                .api_version(vk::make_api_version(0, 1, 3, 0));

            let create_flags = if cfg!(target_os = "macos") {
                vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
            } else {
                vk::InstanceCreateFlags::default()
            };

            let instance_create_info = vk::InstanceCreateInfo::builder()
                .application_info(&app_info)
                .enabled_layer_names(&layer_names_raw) // validation layer, etc
                .enabled_extension_names(&extension_names)
                .flags(create_flags);

            let instance = entry
                .create_instance(&instance_create_info, None)
                .expect("failed to create instance");

            // debug shite for validation layer
            let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(vulkan_debug_callback));

            let debug_utils_loader = DebugUtils::new(&entry, &instance);
            let debug_call_back = debug_utils_loader
                .create_debug_utils_messenger(&debug_info, None)
                .unwrap();
            let surface = ash_window::create_surface(
                &entry,
                &instance,
                window.raw_display_handle(),
                window.raw_window_handle(),
                None,
            )
            .unwrap();

            let physical_devices = instance
                .enumerate_physical_devices()
                .expect("Failed to enumerate physical devices");

            let surface_loader = Surface::new(&entry, &instance);

            let (physical_device, queue_family_index) = physical_devices
                .iter()
                .find_map(|pdevice| {
                    instance
                        .get_physical_device_queue_family_properties(*pdevice)
                        .iter()
                        .enumerate()
                        .find_map(|(index, info)| {
                            let supports_graphic_and_surface =
                                info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                    && surface_loader
                                        .get_physical_device_surface_support(
                                            *pdevice,
                                            index as u32,
                                            surface,
                                        )
                                        .unwrap();
                            if supports_graphic_and_surface {
                                Some((*pdevice, index as u32))
                            } else {
                                None
                            }
                        })
                })
                .expect("Couldn't find suitable device.");

            let device_extension_names_raw = [
                Swapchain::name().as_ptr(),
                #[cfg(target_os = "macos")]
                KhrPortabilitySubsetFn::name().as_ptr(),
            ];

            let features = vk::PhysicalDeviceFeatures {
                shader_clip_distance: 1,
                ..Default::default()
            };
            let priorities = [1.0];

            let queue_info = vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(queue_family_index)
                .queue_priorities(&priorities);

            let device_create_info = DeviceCreateInfo::builder()
                .queue_create_infos(std::slice::from_ref(&queue_info))
                .enabled_extension_names(&device_extension_names_raw)
                .enabled_features(&features);

            // the None is for the allocation callbacks
            // if you want to use your own allocation thing pass that in
            let device: Device = instance
                .create_device(physical_device, &device_create_info, None)
                .unwrap();

            let command_submit_queue = device.get_device_queue(queue_family_index, 0);

            let surface_format = surface_loader
                .get_physical_device_surface_formats(physical_device, surface)
                .unwrap()[0];

            let surface_capabilities = surface_loader
                .get_physical_device_surface_capabilities(physical_device, surface)
                .unwrap();

            let mut desired_image_count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.max_image_count > 0
                && desired_image_count > surface_capabilities.max_image_count
            {
                desired_image_count = surface_capabilities.max_image_count;
            }

            let surface_resolution = match surface_capabilities.current_extent.width {
                std::u32::MAX => vk::Extent2D {
                    width: window_width,
                    height: window_height,
                },
                _ => surface_capabilities.current_extent,
            };

            // if the image will be rotated 90, 270, etc degrees before being presented
            // so if people have tilted monitors it shows correctly
            let pre_transform = if surface_capabilities
                .supported_transforms
                .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                surface_capabilities.current_transform
            };

            // gets supported present modes
            let present_modes = surface_loader
                .get_physical_device_surface_present_modes(physical_device, surface)
                .unwrap();

            // picks the best one (mailbox) if thats not available it goes with fifo
            // because every device has to support fifo
            let present_mode = present_modes
                .iter()
                .cloned()
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(vk::PresentModeKHR::FIFO);

            let swapchain_loader = Swapchain::new(&instance, &device);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface)
                .min_image_count(desired_image_count)
                .image_color_space(surface_format.color_space)
                .image_format(surface_format.format) //  the image and surface format have to match
                .image_extent(surface_resolution) // size of the image
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT) // this is like saying "where is the data going to come from".
                // color attachment means that the image will be rendered directly into by the graphics pipline
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(pre_transform) // transformation before it is presented ex: rotate 90 degrees
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE) // how the alpha channel is treated (transparency), so having it opaque means no transparency at all
                .present_mode(present_mode)
                .clipped(true) // wether shite you cant see if discarded or not
                .image_array_layers(1); // kind of like layers in photoshop

            let swapchain = swapchain_loader
                .create_swapchain(&swapchain_create_info, None) // the none is saying no custom allocation callback
                .unwrap();

            let pool_create_info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER) // this lets the command pool know that you beed to be able to reset your buffers
                .queue_family_index(queue_family_index); // so i can make command buffers taylored to your needs

            let command_pool = device.create_command_pool(&pool_create_info, None).unwrap();

            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
                .command_buffer_count(2)
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY); // there are primary and secondary levels of execution. primary is directly submitted to the queues
                                                         // secondary are analogous to helper functions

            let command_buffers = device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap();

            // there is no actual difference between the command buffers
            // they are just assigned different roles by the programmer to make stuff more organized
            let setup_command_buffer = command_buffers[0];
            let draw_command_buffer = command_buffers[1];

            // NOTE
            // its not present images like current images
            // its images that are presented

            // gets a list of images that can be presented from the swapchain
            let present_images = swapchain_loader.get_swapchain_images(swapchain).unwrap();

            // image views are like buffer views
            let present_image_views: Vec<vk::ImageView> = present_images
                .iter()
                .map(|&image| {
                    // the image view contains metadata about the image
                    let create_view_info = vk::ImageViewCreateInfo::builder()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::R,
                            g: vk::ComponentSwizzle::G,
                            b: vk::ComponentSwizzle::B,
                            a: vk::ComponentSwizzle::A,
                        })
                        // the subresource range are the different sub resources that you will be involving with operations
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR, // an aspect mask is used to specify what specific aspect of an image is used in an operation or view
                            // its called a mask because in the original c++ vulkan is a bitmask but in ash it turns to an enum
                            base_mip_level: 0, // uses full resolution textures as the baseline. if I set this to one then it would use
                            // worse resolution textures as the baseline
                            level_count: 1, // how many mipmap levels it uses. for example, 1 means that the textures are full quality all the time
                            // 2 means that they only go down in quality once, etc
                            base_array_layer: 0, // which array layer the subresource starts from
                            layer_count: 1,      // how many layers are used in the operation/view
                        })
                        .image(image);
                    device.create_image_view(&create_view_info, None).unwrap()
                })
                .collect();

            // represents info about the memory of the device
            let device_memory_properties =
                instance.get_physical_device_memory_properties(physical_device);
            let depth_image_create_info = vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::D16_UNORM) // 16 bit unsigned normalized depth component
                .extent(surface_resolution.into()) // how big the image should be
                .mip_levels(1) // amount of mip levels the depth image contains
                .array_layers(1) // says that it has one layer
                // if it was set higher each layer could contain things of different perspectives
                .samples(vk::SampleCountFlags::TYPE_1) // multi sampling, in this case each pixel is sampled once,
                // which means no anti aliasing
                .tiling(vk::ImageTiling::OPTIMAL) // stores the image in a tiled way
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                // this usage flag says that the depth buffer will have an associated stencil buffer
                // the stencil buffer just holds an additional value for each pixel on the screen
                // it is need for advanced rendering techniques that require more info
                // the value could be anything and you could interperet it as anything
                // all that matters is that it holds an additional value
                .sharing_mode(vk::SharingMode::EXCLUSIVE); // cant be shared between queues

            // the depth image is the same as a depth buffer with a dumb azz name
            let depth_image = device.create_image(&depth_image_create_info, None).unwrap();

            // memory requirements are things like size, alignment, and memory type bits
            // memory type bits contains a bit set to 1 for every supported memory type for the resource
            let depth_image_memory_req = device.get_image_memory_requirements(depth_image);

            // the memory index is like the queue family index. you find the type of memory that is able to store your
            // resource, then from the compatable types you can choose.
            // we need this because we have to allocate the image
            let depth_image_memory_index = find_memory_type_index(
                &depth_image_memory_req,
                &device_memory_properties,
                vk::MemoryPropertyFlags::DEVICE_LOCAL
            )
                .expect("cant find suitable memory index for the depth image (depth buffer dumb azz name bruh)");

            let depth_image_allocate_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(depth_image_memory_req.size)
                .memory_type_index(depth_image_memory_index);

            let depth_image_memory = device
                .allocate_memory(&depth_image_allocate_info, None)
                .unwrap();

            // the 0 is the offset
            // lets say i want to have different resources in that memory reigon, i can
            // make an offset for those
            // resource a starts at 0, resource b starts at 100
            device
                .bind_image_memory(depth_image, depth_image_memory, 0)
                .expect("cant bind the depth image to the memory allocated for it");

            let fence_create_info =
                vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);

            let draw_command_buffer_reuse_fence = device
                .create_fence(&fence_create_info, None)
                .expect("failed to make draw command reuse fence");

            let setup_command_buffer_reuse_fence = device
                .create_fence(&fence_create_info, None)
                .expect("failed to create setup commands reuse fence");

            record_submit_commandbuffer(
                &device,
                setup_command_buffer,
                setup_command_buffer_reuse_fence,
                command_submit_queue,
                &[],
                &[],
                &[],
                // the closure that has the commands you want to record
                |device, setup_command_buffer| {
                    let layout_transition_barriers = vk::ImageMemoryBarrier::builder()
                        .image(depth_image)
                        // dst meaning destination
                        .dst_access_mask(
                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        // a sub resource range is used in the creation of a memory barrier because it
                        // tells what subresources the barrier will affect
                        .subresource_range(
                            vk::ImageSubresourceRange::builder()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .layer_count(1)
                                .level_count(1)
                                .build(),
                        );

                    // this is the actuall command that will be recorded into the buffer
                    // this command is to insert a memory dependency
                    // bruh
                    device.cmd_pipeline_barrier(
                        setup_command_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[layout_transition_barriers.build()],
                    )
                },
            );

            let depth_image_view_info = vk::ImageViewCreateInfo::builder()
                // specifiy which sub resources the view will be viewing
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .level_count(1)
                        .layer_count(1)
                        .build(),
                )
                .image(depth_image)
                .format(depth_image_create_info.format)
                .view_type(vk::ImageViewType::TYPE_2D);

            let depth_image_view = device
                .create_image_view(&depth_image_view_info, None)
                .unwrap();

            let semaphore_create_info = vk::SemaphoreCreateInfo::default();

            // this name is kinda bad
            // its not present (complete semaphore)
            // its (present complete) semaphore
            // as in making a semaphore for when presenting is complete
            let present_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();

            // same thing with the dumb name
            let rendering_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();
            Self {
                event_loop: RefCell::new(event_loop),
                entry,
                instance,
                device,
                queue_family_index,
                physical_device,
                device_memory_properties,
                window,
                surface_loader,
                surface_format,
                command_submit_queue,
                surface_resolution,
                swapchain_loader,
                swapchain,
                present_images,
                present_image_views,
                command_pool,
                draw_command_buffer,
                setup_command_buffer,
                depth_image,
                depth_image_view,
                present_complete_semaphore,
                rendering_complete_semaphore,
                draw_command_buffer_reuse_fence,
                setup_command_buffer_reuse_fence,
                surface,
                debug_call_back,
                debug_utils_loader,
                depth_image_memory,
            }
        }
    }
}

impl Drop for Base {
    fn drop(&mut self) {
        unsafe {
            // DESTROYYYYY

            // this is not telling the device to become idle
            // you are waiting for it to become idle
            self.device.device_wait_idle().unwrap();

            // now that everything is done we can destroy
            self.device
                .destroy_semaphore(self.present_complete_semaphore, None);
            self.device
                .destroy_semaphore(self.rendering_complete_semaphore, None);
            self.device
                .destroy_fence(self.draw_command_buffer_reuse_fence, None);
            self.device
                .destroy_fence(self.setup_command_buffer_reuse_fence, None);

            // just like you alloc memory seperatly then bind the image to it
            // you destroy the memory and destroy the image/view seperatly
            self.device.free_memory(self.depth_image_memory, None);
            self.device.destroy_image_view(self.depth_image_view, None);
            self.device.destroy_image(self.depth_image, None);
            for &image_view in self.present_image_views.iter() {
                self.device.destroy_image_view(image_view, None);
            }
            self.device.destroy_command_pool(self.command_pool, None);
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None); // i used the device to destroy the device
            self.surface_loader.destroy_surface(self.surface, None);
            self.debug_utils_loader
                .destroy_debug_utils_messenger(self.debug_call_back, None);
            self.instance.destroy_instance(None);
        }
    }
}
