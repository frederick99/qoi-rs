#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>

#define RGFW_OPENGL
#define RGFW_IMPLEMENTATION
#include "RGFW.h"

#define RGL_LOAD_IMPLEMENTATION
#include "rglLoad.h"

#define SHADER_VERSION "#version 330 core"
#include "utils.c"

const char* vertexShaderSource = SHADER_SRC(
    layout (location = 0) in vec3 aPos;
    layout (location = 1) in vec2 aTexCoord;
    out vec2 texCoord;
    uniform mat4 modelMatrix;
    uniform mat4 projMatrix;
    void main()
    {
        gl_Position = modelMatrix * projMatrix * vec4(aPos, 1.0);
        texCoord = aTexCoord;
    });

const char* fragmentShaderSource = SHADER_SRC(
    out vec4 FragColor;
    in vec2 texCoord;
    uniform sampler2D myTexture;
    void main()
    {
        vec2 c = texCoord;
        FragColor = mix(vec4(c, 0.0f, 1.0f), texture(myTexture, c), 1.0);
    });

unsigned char* qoi_decode(const char* data, size_t len, size_t* width, size_t* height);
void qoi_free(char* data);

int main(int argc, char **argv) {
    if (argc < 2) {
        printf("usage: %s <input file>\n", argv[0]);
        return 1;
    }

    char* file_path = argv[1];
    StringBuffer sb = {0};
    if (!read_entire_file(file_path, &sb)) {
        printf("Error reading the file %s\n", file_path);
        return 1;
    }

    size_t width, height;
    unsigned char* image = qoi_decode(sb.data, sb.count, &width, &height);
    free(sb.data);

    if (!image) {
        printf("Failed to decode. Make sure image is valid QOI format.\n");
        return -1;
    }

    // WINDOW
    RGFW_glHints* hints = RGFW_getGlobalHints_OpenGL();
    hints->major = 3;
    hints->minor = 3;
    RGFW_setGlobalHints_OpenGL(hints);

    RGFW_window* window = RGFW_createWindow("QOI Viewer", 100, 100, 500, 500, RGFW_windowAllowDND | RGFW_windowCenter | RGFW_windowScaleToMonitor | RGFW_windowOpenGL);
    if (window == NULL)
    {
        printf("Failed to create RGFW window\n");
        return -1;
    }
    RGFW_window_setExitKey(window, RGFW_escape);
    RGFW_window_makeCurrentContext_OpenGL(window);

    if (RGL_loadGL3((RGLloadfunc)RGFW_getProcAddress_OpenGL)) {
        printf("Failed to initialize GLAD\n");
        return -1;
    }

    // SHADERS
    GLint vertexShader = glCreateShader(GL_VERTEX_SHADER);
    glShaderSource(vertexShader, 1, &vertexShaderSource, NULL);
    glCompileShader(vertexShader);

    int success;
    char infoLog[512];
    glGetShaderiv(vertexShader, GL_COMPILE_STATUS, &success);
    if (!success)
    {
        glGetShaderInfoLog(vertexShader, 512, NULL, infoLog);
        printf("ERROR::SHADER::VERTEX::COMPILATION_FAILED\n%s\n", infoLog);
    }

    GLint fragmentShader = glCreateShader(GL_FRAGMENT_SHADER);
    glShaderSource(fragmentShader, 1, &fragmentShaderSource, NULL);
    glCompileShader(fragmentShader);

    glGetShaderiv(fragmentShader, GL_COMPILE_STATUS, &success);
    if (!success)
    {
        glGetShaderInfoLog(fragmentShader, 512, NULL, infoLog);
        printf("ERROR::SHADER::FRAGMENT::COMPILATION_FAILED\n%s\n", infoLog);
    }

    GLint shaderProgram = glCreateProgram();
    glAttachShader(shaderProgram, vertexShader);
    glAttachShader(shaderProgram, fragmentShader);
    glLinkProgram(shaderProgram);

    glGetProgramiv(shaderProgram, GL_LINK_STATUS, &success);
    if (!success) {
        glGetProgramInfoLog(shaderProgram, 512, NULL, infoLog);
        printf("ERROR::SHADER::PROGRAM::LINKING_FAILED\n%s\n", infoLog);
    }
    glDeleteShader(vertexShader);
    glDeleteShader(fragmentShader);

    // VERTEX DATA
    float aspect_image = (float) width / (float) height;
    float top = 1.0, bottom = -1.0, left = -1.0, right = 1.0;
    if (aspect_image >= 1.0) {
        top = 1.0 / aspect_image;
        bottom = -1.0 / aspect_image;
    } else {
        right = aspect_image;
        left = -aspect_image;
    }
    Vertex vertices[] = {
        { .pos = { right, top,    0.0f }, .tex = { 1.0f, 0.0f } },
        { .pos = { right, bottom, 0.0f }, .tex = { 1.0f, 1.0f } },
        { .pos = { left,  bottom, 0.0f }, .tex = { 0.0f, 1.0f } },
        { .pos = { left,  top,    0.0f }, .tex = { 0.0f, 0.0f } },
    };
    unsigned int indices[] = {
        0, 1, 3,  // first triangle
        1, 2, 3,  // second triangle
    };

    GLint VBO, VAO, EBO;
    glGenVertexArrays(1, &VAO);
    glGenBuffers(1, &VBO);
    glGenBuffers(1, &EBO);
    glBindVertexArray(VAO);

    glBindBuffer(GL_ARRAY_BUFFER, VBO);
    glBufferData(GL_ARRAY_BUFFER, sizeof(vertices), vertices, GL_STATIC_DRAW);

    glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, EBO);
    glBufferData(GL_ELEMENT_ARRAY_BUFFER, sizeof(indices), indices, GL_STATIC_DRAW);

    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(Vertex), (void*)offsetof(Vertex, pos));
    glEnableVertexAttribArray(0);

    glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE, sizeof(Vertex), (void*)offsetof(Vertex, tex));
    glEnableVertexAttribArray(1);

    // TEXTURE
    GLint texture;
    glGenTextures(1, &texture);

    glBindTexture(GL_TEXTURE_2D, texture);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);

    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, width, height, 0, GL_RGBA, GL_UNSIGNED_BYTE, image);
    glGenerateMipmap(GL_TEXTURE_2D);

    qoi_free(image);

    // MAIN LOOP
    bool is_pressed = false;
    int mouse_x, mouse_y;
    int saved_mouse_x, saved_mouse_y;

    Matrix model = {0}, saved_model;
    Matrix proj = {0};

    matrix_load_identity(&model);
    matrix_load_identity(&proj);

    float aspect_ratio = (float)window->w / (float)window->h;
    if (aspect_image < aspect_ratio)
        matrix_scale(&proj, 1.0/aspect_ratio, 1.0, 1.0);
    else
        matrix_scale(&proj, 1.0, aspect_ratio/1.0, 1.0);

    glUseProgram(shaderProgram);
    GLint model_loc = glGetUniformLocation(shaderProgram, "modelMatrix");
    GLint proj_loc = glGetUniformLocation(shaderProgram, "projMatrix");

    RGFW_event event;
    bool is_first_run = true;
    while (RGFW_window_shouldClose(window) == RGFW_FALSE) {

        bool update_model = is_first_run;
        bool update_proj = is_first_run;
        is_first_run = false;

        while (RGFW_window_checkEvent(window, &event)) {
            if (event.type == RGFW_quit) break;

            else if (event.type == RGFW_windowResized) {
                glViewport(0, 0, window->w, window->h);
                aspect_ratio = (float)window->w / (float)window->h;

                matrix_load_identity(&proj);
                if (aspect_image < aspect_ratio)
                    matrix_scale(&proj, 1.0/aspect_ratio, 1.0, 1.0);
                else
                    matrix_scale(&proj, 1.0, aspect_ratio/1.0, 1.0);
                update_proj = true;
            }

            else if (event.type == RGFW_mouseScroll) {
                float scroll_y = event.scroll.y;
                float current_scale = model.c0.r0;
                if (scroll_y < 0.0 && current_scale < 0.1 ||
                    scroll_y > 0.0 && current_scale > 10.0) {
                } else {
                    float scale_factor = 1.0 + 0.1*scroll_y;
                    matrix_scale(&model, scale_factor, scale_factor, 1.0);
                    update_model = true;
                }
            }

            else if (event.type == RGFW_mouseButtonPressed) {
                if (event.button.value == RGFW_mouseLeft) {
                    is_pressed = true;
                    saved_model = model;
                    saved_mouse_x = mouse_x;
                    saved_mouse_y = mouse_y;
                }
            }

            else if (event.type == RGFW_mouseButtonReleased) {
                if (event.button.value == RGFW_mouseLeft) {
                    is_pressed = false;
                }
            }

            else if (event.type == RGFW_mousePosChanged) {
                mouse_x = event.mouse.x;
                mouse_y = event.mouse.y;
                if (is_pressed) {
                    int dx = mouse_x - saved_mouse_x;
                    int dy = mouse_y - saved_mouse_y;

                    model = saved_model;
                    matrix_translate(&model, (float)dx/window->w*2.0, -(float)dy/window->h*2.0, 0.0);
                    update_model = true;
                }
            }
        }

        glClearColor(0.1f, 0.1f, 0.1f, 1.0f);
        glClear(GL_COLOR_BUFFER_BIT);

        if (update_model) glUniformMatrix4fv(model_loc, 1, GL_FALSE, (const float*)&model);
        if (update_proj) glUniformMatrix4fv(proj_loc, 1, GL_FALSE, (const float*)&proj);

        glDrawElements(GL_TRIANGLES, ARRAY_LEN(indices), GL_UNSIGNED_INT, 0);

        RGFW_window_swapBuffers_OpenGL(window);
    }

    // CLEAN UP
    glDeleteTextures(1, &texture);
    glDeleteVertexArrays(1, &VAO);
    glDeleteBuffers(1, &VBO);
    glDeleteBuffers(1, &EBO);
    glDeleteProgram(shaderProgram);

    RGFW_window_close(window);
    return 0;
}
