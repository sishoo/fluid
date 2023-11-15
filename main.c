#define GLFW_INCLUDE_VULKAN
#include <GLFW/glfw3.h>

#include <stdlib.h>
#include <stdio.h>
#include <stdbool.h>
#include <string.h>

const unsigned int WINDOW_WIDTH = 800;
const unsigned int WINDOW_HEIGHT = 600;

const char* LAYER_NAMES[] = {"VK_LAYER_KHRONOS_validation"};

#ifdef NDEBUG
    const bool ENABLE_VALIDATION_LAYER = false;
#else
    const bool ENABLE_VALIDATION_LAYER = true;
#endif

static void error_callback(int error, const char *description)
{
    fprintf(stderr, "Error: %s\n", description);
}

// the magic command:
// gcc -o fluid_beast main.c -lglfw -lvulkan

char **build_required_extensions(
    uint32_t *total_extension_count,
    uint32_t glfw_extension_count,
    char **glfw_extensions,
    uint32_t requested_extensions_count,
    char **requested_extensions)
{
    uint32_t available_extension_count;
    vkEnumerateInstanceExtensionProperties(NULL, &available_extension_count, NULL);

    VkExtensionProperties *available_extensions = (VkExtensionProperties *)malloc(available_extension_count * sizeof(VkExtensionProperties));
    vkEnumerateInstanceExtensionProperties(NULL, &available_extension_count, &available_extensions);

    uint32_t required_extension_count = glfw_extension_count + requested_extensions_count;
    char **required_extensions_ptr = (char **)malloc(required_extension_count * sizeof(char *));
    char **required_extensions = required_extensions_ptr;
    for (uint32_t i = 0; i < requested_extensions_count; i++)
    {
        bool extension_available = false;
        for (uint32_t j = 0; j < available_extension_count; j++)
        {
            if (!strcmp(*requested_extensions[i], available_extensions[j].extensionName))
            {
                extension_available = true;
            }
        }
        if (extension_available)
        {
            *required_extensions_ptr = requested_extensions[i];
            required_extensions_ptr++;
        }
    }
    free(required_extensions_ptr);
    *total_extension_count = required_extension_count;
    return required_extensions;
}

bool check_validation_layer_support()
{
    uint32_t layer_count;
    vkEnumerateInstanceLayerProperties(&layer_count, NULL);
    VkLayerProperties *available_layers = (VkLayerProperties *)malloc(layer_count * sizeof(VkLayerProperties));
    vkEnumerateInstanceLayerProperties(&layer_count, available_layers);
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

    VkInstance instance;
    if (vkCreateInstance(&instance_create_info, NULL, &instance) != VK_SUCCESS)
    {
        printf("Cannot create the vulkan instance.\n");
        return -1;
    }

    



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