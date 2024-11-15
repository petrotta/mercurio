package io.github.petrotta.mercurio;


import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.plugins.type.TaskPlugin;
import io.github.petrotta.mercurio.plugins.PluginAnnotation;

import static io.github.petrotta.mercurio.Application.console;

@PluginAnnotation(organization="mercurio", name = "complexity", version = "0.0.4")
public class MyPlugin extends TaskPlugin {


    @Override
    public void init() {
        console("*******************************************");
        console("Complexity Analyzer: Init Plugin");
        console("Name: " + getAnnotation().name());
        console("Version: " + getAnnotation().version());
        console("*******************************************");

    }

    @Override
    public void run(Project project) {

        this.getClass().getPackage().getName();

        console("Running complexity analyzer...");

        console("Input Resources: " + project.getInputResources().size());


        console(project.validate().toString());


    }
}
