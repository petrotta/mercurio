package io.github.petrotta.mercurio;


import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.plugins.Plugin;
import io.github.petrotta.mercurio.plugins.PluginAnnotation;

import static io.github.petrotta.mercurio.Application.console;

@PluginAnnotation(name = "complexity", version = "1.0.2")
public class MyPlugin extends Plugin {


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
