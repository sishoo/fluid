use ash::util::{read_spv, Align};
use ash::vk;
pub use ash::{Device, Instance};
use std::default::Default;
use std::ffi::CStr;
use std::io::Cursor;
use std::mem;
use std::mem::align_of;
use std::mem::size_of;

mod lib;
use lib::{find_memory_type_index, compile_glsl_to_spirv, record_submit_commandbuffer, Base};

mod types;
use types::Vec2;


/*
THINGS TO DO:

use a compute shader for the fluid calculations

handle the 2d array confusion.
using an image makes sense for the 2d part but if we want to implement dynamic details it would be wierd
maybe flatted out the 2d buffer into a 1d one then take chunks from it with the size of the cell_resolution


 */




// const WINDOW_WIDTH: u32 = 800;
// const WINDOW_HEIGHT: u32 = 600;
const WINDOW_SIZE_PIXELS: u32 = 1000;
// const CELL_SIZE_PIXELS: usize = 50; // making smaller cells means more accurate but more laggy and vice versa
const CELL_RESOLUTION: usize = 128; // more cells = more accurate = more lag and vice versa
const NUM_CELLS: usize = CELL_RESOLUTION * CELL_RESOLUTION;


#[macro_export]
macro_rules! offset_of {
    ($base:path, $field:ident) => {{
        unsafe {
            let b: $base = mem::zeroed();
            std::ptr::addr_of!(b.$field) as isize - std::ptr::addr_of!(b) as isize
        }
    }};
}

fn main() {
    // the fluid description buffer is a buffer that contains both the
    // pressure values and the vector field
    // apparently its best practice to allocate one buffer, and assign different ranges of that buffer to
    // data if the data is commonly used together
    let fluid_description_buffer_size_bytes = (NUM_CELLS * (size_of::<Vec2>() + size_of::<f32>())) as u64;

    let fluid_description_buffer_create_info = vk::BufferCreateInfo {
        size: fluid_description_buffer_size_bytes,
        usage: vk::BufferUsageFlags::STORAGE | vk::BufferUsageFlags::TRANSFER,
        sharing_mode: vk::SharingMode::CONCURRENT
    };

    let fluid_description_buffer = base
        .device
        .create_buffer(&vector_field_create_info, None)
        .unwrap();

    let fluid_description_buffer_memory_reqs = base
        .device
        .get_buffer_memory_requirements(fluid_description_buffer);

    let fluid_description_buffer_memory_type_index = get_memory_type_index(
        &fluid_description_buffer_memory_reqs,
        &base.device_memory_properties,
        vk::MemoryPropertyFlags::HOST_VISIBLE || vk::MemoryPropertyFlags::HOST_COHERENT
    );

    let fluid_description_buffer_allocate_info = vk::MemoryAllocateInfo {
        allocation_size: fluid_description_buffer_memory_reqs.size,
        memory_type_index: fluid_description_buffer_memory_type_index
    };

    let fluid_description_buffer_memory = base
        .device
        .allocate_buffer(&fluid_description_buffer_allocate_info, None)
        .unwrap();

    let vector_field_default_data = [[Vec2::new(0.0, 0.0); CELL_RESOLUTION]; CELL_RESOLUTION];
    let vector_field_size_bytes = NUM_CELLS * size_of::<Vec2>();

    let pressure_map_default_data = [[0.0; CELL_RESOLUTION]; CELL_RESOLUTION];
    let pressure_map_size_bytes = NUM_CELLS * size_of::<f32>();

    let mut mapped_fluid_description_buffer_ptr = base
        .device
        .map_memory(
            fluid_description_buffer_memory,
            0,
            fluid_description_buffer_memory_reqs.size,
            vk::MemoryMapFlags::empty(),
        )
        .unwrap();

    unsafe {
        // first copy the vector field in
        std::ptr::copy_nonoverlapping(
            vector_field_default_data.as_ptr(),
            mapped_fluid_description_buffer_ptr as *mut Vec2,
            vector_field_default_data.len()
        );
        mapped_fluid_description_buffer_ptr += vector_field_default_data.len();
        // then the pressure map
        std::ptr::copy_nonoverlapping(
            pressure_map_default_data.as_ptr(),
            mapped_fluid_description_buffer_ptr as *mut f32,
            pressure_map_default_data.len()
        );
    }

    unsafe {
        let base = Base::new(WINDOW_SIZE_PIXELS, WINDOW_SIZE_PIXELS, "fluid beast");

        // we have to make a an array of attachments for the renderpass then get reference object things
        // for each attachment in the array
        let renderpass_attachments = [
            // this is the color attachment (stores color)
            vk::AttachmentDescription {
                format: base.surface_format.format,
                samples: vk::SampleCountFlags::TYPE_1, // one sample per pixel
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE, // this will be written into by the graphics pipline
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            },
            // this attachment is the depth stencil
            vk::AttachmentDescription {
                format: vk::Format::D16_UNORM,
                // this is saying how many samples per pixel
                // a sample is  how many points on the pixel are used in the average that determines the pixels color
                samples: vk::SampleCountFlags::TYPE_1,
                // load a cleared version
                load_op: vk::AttachmentLoadOp::CLEAR,
                initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, // tiled memory layout
                final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ..Default::default()
            },
        ];

        // reference object things that say which index of the renderpass
        // attachment array represents what
        let color_attachment_refs = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];

        let depth_attachment_ref = vk::AttachmentReference {
            attachment: 1,
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        };

        // subpass dependencies are things that subpasses depend on
        // this is a way to specify dependencies between subpasses
        let dependencies = [
            // this specific dependency doesnt specifically say what it is dependent on
            // however it does say specific pipeline stages where it is dependent on something
            vk::SubpassDependency {
                // this means that the dependency originates from something outside the current renderpass
                src_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                // dst means destination
                // this is refering to the destination resource
                dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ
                    | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                ..Default::default()
            }
        ];

        let subpass = vk::SubpassDescription::builder()
            .color_attachments(&color_attachment_refs)
            .depth_stencil_attachment(&depth_attachment_ref)
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);

        // this render pass only has one subpass
        let renderpass_create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&renderpass_attachments)
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(&dependencies);

        let renderpass = base
            .device
            .create_render_pass(&renderpass_create_info, None)
            .unwrap();

        // in vulkan the framebuffer is more of an encompacing thing
        // that is accociated with each present image view
        let framebuffers: Vec<vk::Framebuffer> = base
            .present_image_views
            .iter()
            .map(|&present_image_view| {
                // each framebuffer has associated attachments
                let framebuffer_attachments = [present_image_view, base.depth_image_view];
                let framebuffer_create_info = vk::FramebufferCreateInfo::builder()
                    .render_pass(renderpass)
                    .attachments(&framebuffer_attachments)
                    .width(base.surface_resolution.width)
                    .height(base.surface_resolution.height)
                    .layers(1);

                base.device
                    .create_framebuffer(&framebuffer_create_info, None)
                    .unwrap()
            })
            .collect();

        // these are the indexs of the vertex array that contain triangles
        // be careful! you need to keep these as u32
        // let index_buffer_data = [0u32, 1, 2];

        // VK BUFFER CREATE INFO TAKE THE NUMBER OF BYTES
        let index_buffer_create_info = vk::BufferCreateInfo {
            size: index_buffer_data_size_bytes,
            usage: vk::BufferUsageFlags::INDEX_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        // let index_buffer_create_info = vk::BufferCreateInfo::builder()
        //     .size(std::mem::size_of_val(&index_buffer_data) as u64)
        //     .usage(vk::BufferUsageFlags::INDEX_BUFFER)
        //     .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let index_buffer = base.device.create_buffer(&index_buffer_create_info, None).unwrap();

        // its getting what memory requirments the memory needs to hold the buffer
        // it has size IN BYTES, alignment, and memory type
        // memory type bits contains a bit set to 1 for every supported memory type for the resource
        // RETURNS THE SIZE IN BYTESSSSSS NOOOTTTTTT LENGTHHH!!!!!!!!!!!!!!!!
        let index_buffer_memory_req = base.device.get_buffer_memory_requirements(index_buffer);
        let index_buffer_memory_index = find_memory_type_index(
            &index_buffer_memory_req,
            &base.device_memory_properties,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
            .expect("cant find subtable memory type for index buffer");

        let index_buffer_allocate_info = vk::MemoryAllocateInfo {
            allocation_size: index_buffer_data_size_bytes,
            memory_type_index: index_buffer_memory_index,
            ..Default::default()
        };


        let index_buffer_memory = base
            .device
            .allocate_memory(&index_buffer_allocate_info, None)
            .unwrap();

        // map memory creates a temporary connection between the vulkan memory space and
        // the actual memory.  its like having a pointer into the vulkan memory object thing
        assert_eq!(index_buffer_data_size_bytes, index_buffer_memory_req.size);
        let mapped_index_ptr = base
            .device
            .map_memory(
                index_buffer_memory,
                0, // offset
                index_buffer_memory_req.size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap() as *mut u32;
        // println!("{:?}", &index_buffer_data.capacity());
        // assert_eq!((&index_buffer_data.len() * std::mem::size_of::<u32>()) as u64, std::mem::size_of_val(&index_buffer_data) as u64);



        unsafe {
            std::ptr::copy_nonoverlapping(index_buffer_data.as_ptr(), mapped_index_ptr as *mut u32, index_buffer_data.len());
        }

        base.device.unmap_memory(index_buffer_memory);
        base.device
            .bind_buffer_memory(index_buffer, index_buffer_memory, 0)
            .unwrap();

        // since the vertex buffer is going to be interacting with a
        // compute shader and vertex shader we can specify the usage as a vertex buffer
        // AND a storage buffer
        // make sure the sharing mode is concurrent so that it can be shared
        let vertex_buffer_create_info = vk::BufferCreateInfo {
            size: vertex_buffer_data_size_bytes,
            usage: vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::STORAGE,
            sharing_mode: vk::SharingMode::CONCURRENT,
            ..Default::default()
        };

        let vertex_buffer = base
            .device
            .create_buffer(&vertex_buffer_create_info, None)
            .unwrap();

        let vertex_buffer_memory_req = base.device.get_buffer_memory_requirements(vertex_buffer);

        let vertex_buffer_memory_index = find_memory_type_index(
            &vertex_buffer_memory_req,
            &base.device_memory_properties,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
            .expect("cant find memory type for vertex buffer");

        let vertex_buffer_allocation_info = vk::MemoryAllocateInfo {
            allocation_size: vertex_buffer_data_size_bytes,
            memory_type_index: vertex_buffer_memory_index,
            ..Default::default()
        };

        let vertex_buffer_memory = base
            .device
            .allocate_memory(&vertex_buffer_allocation_info, None)
            .unwrap();

        // let vertex_buffer_data = [
        //     Vertex {
        //         pos: [-0.5, 0.5, 0.0, 0.0],
        //         color: [0.0, 1.0, 0.0, 1.0],
        //         normal: [0.0, 0.0, 0.0]
        //     },
        //     Vertex {
        //         pos: [0.5, 0.5, 0.0, 1.0],
        //         color: [0.0, 0.0, 1.0, 1.0],
        //         normal: [0.0, 0.0, 0.0]
        //     },
        //     Vertex {
        //         pos: [0.0, -0.5, 0.0, 1.0],
        //         color: [1.0, 0.0, 0.0, 1.0],
        //         normal: [0.0, 0.0, 0.0]
        //     },
        // ];

        // for index in index_buffer_data.into_iter().step_by(3) {
        //         let mut p1 = vertex_buffer_data[index as usize];
        //         let mut p2 = vertex_buffer_data[(index + 1) as usize];
        //         let mut p3 = vertex_buffer_data[(index + 2) as usize];
        //
        //         let u = subtractVP4(p2.pos, p1.pos);
        //         let v = subtractVP4(p3.pos, p1.pos);
        //
        //         // u and v are [x, y, z, w]
        //         // Nx = UyVz - UzVy
        //         // Ny = UzVx - UxVz
        //         // Nz = UxVy - UyVx
        //         let nx = u[1] * v[2] - u[2] * v[1];
        //         let ny = u[2] * v[0] - u[0] * v[2];
        //         let nz = u[0] * v[1] - u[1] * v[0];
        //
        //         p1.normal = normalizeVP3(addVP3([nx, ny, nz], p1.normal));
        //         p2.normal = normalizeVP3(addVP3([nx, ny, nz], p2.normal));
        //     p3.normal = normalizeVP3(addVP3([nx, ny, nz], p3.normal));
        // }

        // pub unsafe fn map_memory(
        //     &self,
        //     memory: DeviceMemory,
        //     offset: DeviceSize,
        //     size: DeviceSize,
        //     flags: MemoryMapFlags
        // ) -> VkResult<*mut c_void>
        // map memory makes a temporary connection from vulkan memory to regular memory

        let mapped_vert_ptr = base
            .device
            .map_memory(
                vertex_buffer_memory,
                0,
                vertex_buffer_memory_req.size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap();

        unsafe {
            std::ptr::copy_nonoverlapping(vertex_buffer_data.as_ptr(), mapped_vert_ptr as *mut Vertex, vertex_buffer_data.len());
        }


        //
        //
        // assert_eq!(vertex_buffer_data_size_bytes, vertex_buffer_memory_req.size);
        // let mut vert_align = Align::new(
        //     mapped_vert_ptr,
        //     align_of::<Vertex>() as u64,
        //     vertex_buffer_data_size_bytes,
        // );

        // feeding in the data to the mapped memory, which feeds it into the vulkan memory aswell because
        // of the temporary connection
        // vert_align.copy_from_slice(&vertex_buffer_data);


        base.device.unmap_memory(vertex_buffer_memory);
        base.device
            .bind_buffer_memory(vertex_buffer, vertex_buffer_memory, 0) // offset
            .unwrap();





        /*
        let normal_buffer_create_info = vk::BufferCreateInfo {
            size: (std::mem::size_of_val(&index_buffer_data) / 3) as u64,
            usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        let normal_buffer = base
            .device
            .create_buffer(&normal_buffer_create_info, None)
            .unwrap();

        let normal_buffer_memory_reqs = base.device.get_buffer_memory_requirements(normal_buffer);

        let normal_buffer_memory_index = find_memory_type_index(
            &normal_buffer_memory_reqs,
            &base.device_memory_properties,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .unwrap();

        let normal_buffer_allocate_info = vk::MemoryAllocateInfo {
            allocation_size: normal_buffer_memory_reqs.size,
            memory_type_index: normal_buffer_memory_index,
            ..Default::default()
        };

        let normal_buffer_memory = base
            .device
            .allocate_memory(&normal_buffer_allocate_info, None)
            .unwrap();

        let mut normal_buffer_data: Vec<Vec3> =
            Vec::with_capacity(std::mem::size_of_val(&index_buffer_data) / 3);

        for face in index_buffer_data.iter().step_by(3) {
            let a = vertex_buffer_data[*face as usize];
            let b = vertex_buffer_data[*face as usize + 1];
            let c = vertex_buffer_data[*face as usize + 2];

            // cross product is given by:
            // uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx
            let u = Vec3 {
                x: b.pos[0] - a.pos[0],
                y: b.pos[1] - a.pos[1],
                z: b.pos[2] - a.pos[2],
            };

            let v = Vec3 {
                x: c.pos[0] - a.pos[0],
                y: c.pos[1] - a.pos[1],
                z: c.pos[2] - a.pos[2],
            };

            let cross = Vec3 {
                x: u.y * v.z - u.z * v.y,
                y: u.z * v.x - u.x * v.z,
                z: u.x * v.y - u.y * v.x,
            };

            normal_buffer_data.push(cross);
        }

        let norm_ptr = base
            .device
            .map_memory(
                normal_buffer_memory,
                0,
                normal_buffer_memory_reqs.size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap();

        let mut norm_align = Align::new(
            norm_ptr,
            align_of::<Vec3>() as u64,
            normal_buffer_memory_reqs.size,
        );

        norm_align.copy_from_slice(&normal_buffer_data);
        base.device.unmap_memory(normal_buffer_memory);
        base.device
            .bind_buffer_memory(normal_buffer, normal_buffer_memory, 0)
            .unwrap();
         */

        let vertex_shader_src = compile_glsl_to_spirv(
            // r"C:\Users\PCema\IdeaProjects\ash_vulkan\src\shader\vertex.vert",
            "/Users/macfarrell/IdeaProjects/ash_vulkan/src/shader/vertex.vert",
            0,
        )
            .unwrap();

        let fragment_shader_src = compile_glsl_to_spirv(
            // r"C:\Users\PCema\IdeaProjects\ash_vulkan\src\shader\fragment.frag",
            "/Users/macfarrell/IdeaProjects/ash_vulkan/src/shader/fragment.frag",
            1,
        )
            .unwrap();
        let vertex_shader_info = vk::ShaderModuleCreateInfo::builder().code(&vertex_shader_src);
        let fragment_shader_info = vk::ShaderModuleCreateInfo::builder().code(&fragment_shader_src);

        // a shader module is a thin wrapper around shader bytecode
        // they dont get compiled / linked until the pipeline creation
        let vertex_shader_module = base
            .device
            .create_shader_module(&vertex_shader_info, None)
            .unwrap();

        let fragment_shader_module = base
            .device
            .create_shader_module(&fragment_shader_info, None)
            .unwrap();

        // pipeline layout
        let layout_create_info = vk::PipelineLayoutCreateInfo::default();

        let pipeline_layout = base
            .device
            .create_pipeline_layout(&layout_create_info, None)
            .unwrap();

        let shader_entry_name = CStr::from_bytes_with_nul_unchecked(b"main\0");

        // this will be used in the creation of the graphics pipeline
        // it lays out what shaders we will use pretty much
        let shader_stage_create_infos = [
            vk::PipelineShaderStageCreateInfo {
                module: vertex_shader_module,
                p_name: shader_entry_name.as_ptr(),
                stage: vk::ShaderStageFlags::VERTEX,
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                module: fragment_shader_module,
                p_name: shader_entry_name.as_ptr(),
                stage: vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            },
        ];

        // this specifies how the vertex data is layed out in memory
        let vertex_input_binding_descriptions = [vk::VertexInputBindingDescription {
            binding: 0, // since you can have more than one of these
            // this is besically just saying what binding varient it is
            stride: mem::size_of::<Vertex>() as u32, // how far apart the start of a vertex
            // is from the start of the next
            input_rate: vk::VertexInputRate::VERTEX, // this says how vertex data is consumed
            // there are two different ones:
            // vertex - saying that each vertex is read as an independent thing.
            // EX: the position, normal, and color are read independently for each vertex
            // instance - the vertex attrs (pos, color, normal) are read once and reused for
            // each vertex in that instance
        }];

        // this is just saying what each part of the vertex input it
        // this for example is saying the first part is position
        // and the second part is the color
        let vertex_input_attribute_descriptions = [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32A32_SFLOAT,
                offset: offset_of!(Vertex, pos) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(Vertex, color) as u32,
            },
        ];

        // the stuff used to configure the vertex input state of the pipeline
        // we kind of custom setup each stage of the graphics pipeline
        // youd have to do this with every custom graphics pipeline you desire
        let vertex_input_state_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(&vertex_input_attribute_descriptions)
            .vertex_binding_descriptions(&vertex_input_binding_descriptions);

        // used to configure the assembly state of the graphics pipeline
        let vertex_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            ..Default::default()
        };

        // a list containing the viewports with info about the viewports
        let viewports = [vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: base.surface_resolution.width as f32,
            height: base.surface_resolution.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }];

        // these scissors define an area in the framebuffer where rendering is allowed and anything
        // outside that area is not drawn
        let scissors = [base.surface_resolution.into()];

        // the scissors and viewport test parameters
        let viewport_state_info = vk::PipelineViewportStateCreateInfo::builder()
            .scissors(&scissors)
            .viewports(&viewports);

        let rasterization_info = vk::PipelineRasterizationStateCreateInfo {
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            line_width: 1.0,
            polygon_mode: vk::PolygonMode::FILL,
            ..Default::default()
        };

        let multisample_state_info = vk::PipelineMultisampleStateCreateInfo {
            rasterization_samples: vk::SampleCountFlags::TYPE_1,
            ..Default::default()
        };

        // stencil op state represent the configuration for stencil testing
        // Keep means that the stencil values will remain unchanged
        // always means that the stencil test will always pass regardless of the stencil and reference value
        // this specific implementation is a config that guarentees that no changes are made to the stencil buffer
        // and the stencil test always passes
        // this is because the enum things are set to keep, which does nothing and always which always passes
        // noop = no operation
        let noop_stencil_state = vk::StencilOpState {
            fail_op: vk::StencilOp::KEEP, // what happens if the stencil test fails
            pass_op: vk::StencilOp::KEEP, // what happens if the stencil test passes
            depth_fail_op: vk::StencilOp::KEEP, // stencil passes depth fails
            compare_op: vk::CompareOp::ALWAYS, // how the stencil values is compared the with the reference value
            ..Default::default()
        };

        // settings and parameters that control how depth testing happens
        let depth_state_info = vk::PipelineDepthStencilStateCreateInfo {
            depth_test_enable: 1,
            depth_write_enable: 1,
            depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
            front: noop_stencil_state,
            back: noop_stencil_state,
            max_depth_bounds: 1.0,
            ..Default::default()
        };

        // used to configure how color blending is performed for a specific attachment in
        // the pipeline
        let color_blend_attachment_states = [vk::PipelineColorBlendAttachmentState {
            blend_enable: 0, // blending is off for this color attachment
            src_color_blend_factor: vk::BlendFactor::SRC_COLOR, // how much of the src color contributes to the final result SRC_COLOR specifically means that  the src color itself is used as the blend factor
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR, // how much of the destination colo rcontributes
            color_blend_op: vk::BlendOp::ADD, // the src and dst colors are added together
            src_alpha_blend_factor: vk::BlendFactor::ZERO, // like src_color_blend_factor but for transparency
            dst_alpha_blend_factor: vk::BlendFactor::ZERO, // like src_color_blend_factor but for transparency
            alpha_blend_op: vk::BlendOp::ADD, // like the color_blend_op but for transparency
            color_write_mask: vk::ColorComponentFlags::RGBA, // means that all colors are written to the color buffer, none masked out. ex: R would mean only the red is written
        }];

        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op(vk::LogicOp::CLEAR)
            .attachments(&color_blend_attachment_states);

        // info about these can be omited at creation time and set during the recording of the command buffer (changed at draw time)
        // this will cause the config of these to be ignored and you must specify them at draw time
        let dynamic_state = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_info =
            vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_state);

        let graphics_pipeline_create_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_stage_create_infos)
            .vertex_input_state(&vertex_input_state_info)
            .input_assembly_state(&vertex_input_assembly_state_info)
            .viewport_state(&viewport_state_info)
            .rasterization_state(&rasterization_info)
            .multisample_state(&multisample_state_info)
            .depth_stencil_state(&depth_state_info)
            .color_blend_state(&color_blend_state)
            .dynamic_state(&dynamic_state_info)
            .layout(pipeline_layout)
            .render_pass(renderpass)
            .build();

        let graphics_pipelines = base
            .device
            .create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[graphics_pipeline_create_info],
                None,
            )
            .expect("cant create graphics pipelimes");

        let graphics_pipeline = graphics_pipelines[0];

        base.render_loop(|| {
            let (present_index, _) = base
                .swapchain_loader
                .acquire_next_image(
                    base.swapchain,
                    std::u64::MAX,
                    base.present_complete_semaphore,
                    vk::Fence::null(),
                )
                .unwrap();
            let clear_values = [
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.0, 0.0, 0.0, 0.0],
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ];

            let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                .render_pass(renderpass)
                .framebuffer(framebuffers[present_index as usize])
                .render_area(base.surface_resolution.into())
                .clear_values(&clear_values);

            record_submit_commandbuffer(
                &base.device,
                base.draw_command_buffer,
                base.draw_command_buffer_reuse_fence,
                base.command_submit_queue,
                &[base.present_complete_semaphore],
                &[base.rendering_complete_semaphore],
                &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
                |device, draw_command_buffer| {
                    device.cmd_begin_render_pass(
                        draw_command_buffer,
                        &render_pass_begin_info,
                        vk::SubpassContents::INLINE,
                    );
                    device.cmd_bind_pipeline(
                        draw_command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        graphics_pipeline,
                    );
                    device.cmd_set_viewport(draw_command_buffer, 0, &viewports);
                    device.cmd_set_scissor(draw_command_buffer, 0, &scissors);
                    device.cmd_bind_vertex_buffers(draw_command_buffer, 0, &[vertex_buffer], &[0]);
                    device.cmd_bind_index_buffer(
                        draw_command_buffer,
                        index_buffer,
                        0,
                        vk::IndexType::UINT32,
                    );
                    device.cmd_draw_indexed(
                        draw_command_buffer,
                        index_buffer_data.len() as u32,
                        1,
                        0,
                        0,
                        1,
                    );
                    device.cmd_end_render_pass(draw_command_buffer);
                },
            );
            let wait_semaphores = [base.rendering_complete_semaphore];
            let swapchains = [base.swapchain];
            let image_indices = [present_index];
            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&wait_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);

            base.swapchain_loader
                .queue_present(base.command_submit_queue, &present_info)
                .unwrap();
        });

        base.device.device_wait_idle().unwrap();
        for pipeline in graphics_pipelines {
            base.device.destroy_pipeline(pipeline, None);
        }
        base.device.destroy_pipeline_layout(pipeline_layout, None);
        base.device
            .destroy_shader_module(vertex_shader_module, None);
        base.device
            .destroy_shader_module(fragment_shader_module, None);
        base.device.free_memory(index_buffer_memory, None);
        base.device.destroy_buffer(index_buffer, None);
        base.device.free_memory(vertex_buffer_memory, None);
        base.device.destroy_buffer(vertex_buffer, None);
        // base.device.free_memory(normal_buffer_memory, None);
        // base.device.destroy_buffer(normal_buffer, None);
        for framebuffer in framebuffers {
            base.device.destroy_framebuffer(framebuffer, None);
        }
        base.device.destroy_render_pass(renderpass, None);
    }
}
