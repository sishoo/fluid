#define GLFW_INCLUDE_VULKAN
#include <GLFW/glfw3.h>

#include <stdlib.h>
#include <stdio.h>

const unsigned int WINDOW_WIDTH = 800;
const unsigned int WINDOW_HEIGHT = 600;

static void error_callback(int error, const char* description) {
    fprintf(stderr, "Error: %s\n", description);
}

// the magic command:
// gcc -o my_program main.c -lglfw

int main() {
    /*
    The flow of logic is:
    init window
    init vulkan
    run main loop
    dealloc stuff
    */

    GLFWwindow* window;


    glfwSetErrorCallback(error_callback);

    // init glfw
    if (!glfwInit()) {
        glfwTerminate();
        return -1;
    }

    // init the window
    glfwWindowHint(GLFW_CLIENT_API, GLFW_NO_API); // saying not to make opengl context
    glfwWindowHint(GLFW_RESIZABLE, GLFW_FALSE);
    window = glfwCreateWindow(WINDOW_WIDTH, WINDOW_HEIGHT, "Cool window", NULL, NULL);
    if (!window) {
        glfwTerminate();
        return -1;
    }

    // init vulkan
    VkApplicationInfo app_info = {};
    app_info.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO;
    app_info.pApplicationName = "BEAST";
    app_info.applicationVersion = VK_MAKE_VERSION(1, 0, 0);




    // main loop
    while (!glfwWindowShouldClose(window)) {
        glfwPollEvents();
    }


    // cleanup
    glfwDestroyWindow(window);
    glfwTerminate();

    return 0;
}