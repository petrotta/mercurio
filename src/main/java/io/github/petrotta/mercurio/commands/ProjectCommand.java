package io.github.petrotta.mercurio.commands;

import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.build.StructuredProject;
import io.github.petrotta.mercurio.build.UnstructuredProject;

import java.io.File;

import static io.github.petrotta.mercurio.Application.console;

public abstract class ProjectCommand {

    public Project initProject(File sourceDir, File libDir, boolean verbose) throws Exception {
        if(sourceDir == null) {
            sourceDir = Application.getCurrentDir();;
        }
        console("Source Dir:   "+ sourceDir.toString(), verbose);

        if(libDir == null) {
            libDir = Application.getStdLibDir();
        }
        console("Library Dir: " + libDir.getAbsolutePath(), verbose);

        if(StructuredProject.containsManifest(sourceDir)) {
            console("Structured project recognized", verbose);
            return new StructuredProject(sourceDir, libDir);
        }  else {
            console("Unstructured project recognized", verbose);
            return new UnstructuredProject(sourceDir, libDir);
        }

    }


}
