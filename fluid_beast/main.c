#define GLFW_INCLUDE_VULKAN
#include <GLFW/glfw3.h>

#include <stdlib.h>
#include <stdio.h>
#include <stdbool.h>
#include <string.h>

const unsigned int WINDOW_WIDTH = 800;
const unsigned int WINDOW_HEIGHT = 600;

const char *LAYER_NAMES[] = {"VK_LAYER_KHRONOS_validation"};

#ifdef NDEBUG
const bool ENABLE_VALIDATION_LAYERS = false;
#else
const bool ENABLE_VALIDATION_LAYERS = true;
#endif

static void error_callback(int error, const char *description)
{
    fprintf(stderr, "Error: %s\n", description);
}

// the magic command:
// gcc -o fluid_beast main.c -lglfw -lvulkan

char **build_required_extensions(
    uint32_t *total_extensions_count,
    uint32_t glfw_extensions_count,
    char **glfw_extensions,
    uint32_t requested_extensions_count,
    char **requested_extensions)
{
    uint32_t available_extension_count;
    vkEnumerateInstanceExtensionProperties(NULL, &available_extension_count, NULL);
    VkExtensionProperties *available_extensions = (VkExtensionProperties *)malloc(available_extension_count * sizeof(VkExtensionProperties));
    vkEnumerateInstanceExtensionProperties(NULL, &available_extension_count, available_extensions);

    uint32_t max_possible_extensions_count = glfw_extensions_count + requested_extensions_count;

#if ENABLE_VALIDATION_LAYERS
    max_possible_extensions_count++;
#endif

    char **final_extensions_buffer_ptr = (char **)malloc(max_possible_extensions_count * sizeof(char *));

#if ENABLE_VALIDATION_LAYERS
    *final_externsions_buffer_ptr = VK_EXT_DEBUG_UTILS_EXTENSION_NAME;
    final_requested_extensions_ptr++;
#endif

    char **final_extensions_buffer_ptr_start = final_extensions_buffer_ptr;
    uint32_t final_extensions_count = 0;
    for (uint32_t i = 0; i < glfw_extensions_count; i++)
    {
        for (uint32_t j = 0; j < available_extension_count; j++)
        {
            if (strcmp(glfw_extensions[i], available_extensions[j].extensionName))
            {
                continue;
            }
            *final_extensions_buffer_ptr = glfw_extensions[i];
            final_extensions_buffer_ptr++;
            final_extensions_count++;
        }
    }

    for (uint32_t i = 0; i < requested_extensions_count; i++)
    {
        for (uint32_t j = 0; j < available_extension_count; j++)
        {
            if (strcmp(requested_extensions[i], available_extensions[j].extensionName))
            {
                continue;
            }
            *final_extensions_buffer_ptr = requested_extensions[i];
            final_extensions_buffer_ptr++;
            final_extensions_count++;
        }
    }

    *total_extensions_count = final_extensions_count;
    char **final_extensions_buffer = (char **)realloc(final_extensions_buffer_ptr_start, final_extensions_count * sizeof(char *));
    return final_extensions_buffer;
}

bool check_validation_layer_support(char *validation_layers[])
{
    uint32_t layer_count;
    vkEnumerateInstanceLayerProperties(&layer_count, NULL);
    VkLayerProperties *available_layers = (VkLayerProperties *)malloc(layer_count * sizeof(VkLayerProperties));
    vkEnumerateInstanceLayerProperties(&layer_count, available_layers);

    for (uint32_t i = 0; i < sizeof(validation_layers) / sizeof(char *); i++)
    {
        bool layer_available = false;
        for (uint32_t j = 0; j < layer_count; j++)
        {
            if (!strcmp(validation_layers[i], available_layers[j].layerName))
            {
                layer_available = true;
            }
        }
        if (!layer_available) {
            return false;
        }
    }
    return true;
}

// static bool debug_callback(

// ) {
//     char* final_debug_message = (char *)malloc();

//     FILE* file_ptr = fopen("./log.txt", "w");
//     if (!file_ptr)
//     {
//         printf("Cannot open log file, writing debug output to stdout only.\n");
//         // print to stdout
//         return false;
//     }

//     fprintf(file_ptr, final_debug_message);
//     fclose(file_ptr);



//     return false;
// }

VkPhysicalDevice pick_best_physical_device(uint32_t device_count, VkPhysicalDevice *devices_buffer) {
    if (device_count == 1)
    {
        return devices_buffer[0];
    }

    // kind of wack but look at the vulkan spec for vkPhysicalDeviceType.
    // you index in with the number that represents the device type and it returns the heigharchy value
    // 1 is best 5 is worst
    uint32_t device_type_heigharchy_table[5] = {5, 2, 1, 3, 4};

    VkPhysicalDevice best_device;
    uint32_t best_device_heigharchy_spot = 100;
    for (uint32_t i = 0; i < device_count; i++)
    {
        VkPhysicalDevice device = devices_buffer[i];

        VkPhysicalDeviceProperties device_properties;
        VkPhysicalDeviceFeatures device_features;

        vkGetPhysicalDeviceProperties(device, &device_properties);
        vkGetPhysicalDeviceFeatures(device, &device_features);
        




    }

}

int main()
{

    /*
    The flow of logic is:
    init window
    init vulkan
    run main loop
    dealloc stuff
    */

    glfwSetErrorCallback(error_callback);

    // init glfw
    if (!glfwInit())
    {
        glfwTerminate();
        return -1;
    }

    // init the window
    glfwWindowHint(GLFW_CLIENT_API, GLFW_NO_API); // saying not to make opengl context
    glfwWindowHint(GLFW_RESIZABLE, GLFW_FALSE);
    GLFWwindow *window = glfwCreateWindow(WINDOW_WIDTH, WINDOW_HEIGHT, "Cool window", NULL, NULL);
    if (!window)
    {
        glfwTerminate();
        return -1;
    }


    // init vulkan
    VkApplicationInfo app_info = {};
    app_info.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO;
    app_info.pApplicationName = "BEAST";
    app_info.applicationVersion = VK_MAKE_VERSION(1, 0, 0);
    app_info.pEngineName = "no engine";
    app_info.engineVersion = VK_MAKE_VERSION(1, 0, 0);
    app_info.apiVersion = VK_API_VERSION_1_0;


    VkInstanceCreateInfo instance_create_info = {};
    instance_create_info.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO;
    instance_create_info.pApplicationInfo = &app_info;
    instance_create_info.enabledLayerCount = 0;
    instance_create_info.flags |= VK_INSTANCE_CREATE_ENUMERATE_PORTABILITY_BIT_KHR;
    uint32_t glfw_extension_count = 0;
    const char **glfw_extensions = glfwGetRequiredInstanceExtensions(&glfw_extension_count);
    uint32_t requested_extension_count = 1;
    const char *requested_extensions[] = {VK_KHR_PORTABILITY_ENUMERATION_EXTENSION_NAME};
    uint32_t required_extension_count = 0;
    char **required_extensions = build_required_extensions(&required_extension_count, glfw_extension_count, glfw_extensions, requested_extension_count, requested_extensions);
    instance_create_info.enabledExtensionCount = required_extension_count;
    instance_create_info.ppEnabledExtensionNames = required_extensions;
    const char *validation_layers[] = {"VK_LAYER_KHRONOS_validation"};
    if (ENABLE_VALIDATION_LAYERS && !check_validation_layer_support(validation_layers))
    {
        printf("Some of the validation layers requested are not available.\n");
        return -1;
    }
    if (ENABLE_VALIDATION_LAYERS)
    {
        instance_create_info.enabledLayerCount = sizeof(validation_layers) / sizeof(char *);
        instance_create_info.ppEnabledLayerNames = validation_layers;
    }
    else
    {
        instance_create_info.enabledLayerCount = 0;
    }


    VkInstance instance;
    if (vkCreateInstance(&instance_create_info, NULL, &instance) != VK_SUCCESS)
    {
        printf("Cannot create the vulkan instance.\n");
        return -1;
    }


    VkPhysicalDevice physical_device = VK_NULL_HANDLE;
    uint32_t physical_device_count = 0;
    vkEnumeratePhysicalDevices(instance, &physical_device_count, NULL);
    if (physical_device_count == 0) 
    {
        printf("Cannot find a physical device that supports Vulkan.");
        return -1;
    }
    VkPhysicalDevice physical_devices_buffer = (VkPhysicalDevice *)malloc(physical_device_count * sizeof(VkPhysicalDevice));
    vkEnumeratePhysicalDevices(instance, &physical_device_count, physical_devices_buffer);
    VkPhysicalDevice physical_device = pick_best_physical_device(physical_devices_buffer);






    // main loop
    while (!glfwWindowShouldClose(window))
    {
        glfwPollEvents();
    }

    // cleanup
    vkDestroyInstance(instance, NULL);
    glfwDestroyWindow(window);
    glfwTerminate();

    return 0;
}