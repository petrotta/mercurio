package io.github.petrotta.mercurio.build;

import io.github.petrotta.mercurio.build.xml.PackageManifest;
import lombok.Getter;
import org.gradle.tooling.GradleConnector;
import org.gradle.tooling.ProjectConnection;
import org.gradle.tooling.model.Task;
import org.xml.sax.SAXException;

import java.io.File;
import java.io.IOException;

import static io.github.petrotta.mercurio.Application.console;

public class GradleProject extends Project {
    public static final String MANIFEST_FILE = "build.gradle";


    public static final String FOLDER_SRC = "src";
    public static final String FOLDER_TEST = "tests";
    private static final String FOLDER_LIBS = "libs";
    private static final String FOLDER_BUILD = "build";
    @Getter private PackageManifest manifest;

    public GradleProject(File dir, File library) throws IOException, SAXException {
        this(dir,library, false);
    }
    public GradleProject(File dir, File library, boolean verbose) throws IOException, SAXException {
        super(dir, library, verbose);
         manifest = BuildSystem.readBuildFile(new File(dir, MANIFEST_FILE));
         printTasks();
    }

    public static boolean containsManifest(File dir) {
        return new File(dir, MANIFEST_FILE).exists();
    }

    @Override
    public File getSourceDir() {
        return new File(this.getDir(), FOLDER_SRC);
    }

    public void printTasks()  {

        GradleConnector connector = GradleConnector.newConnector();
        connector.forProjectDirectory(new File(".")); // project dir

        ProjectConnection connection = connector.connect();
        try {
            // Load the model for the project
            org.gradle.tooling.model.GradleProject project = connection.getModel(org.gradle.tooling.model.GradleProject.class);
            System.out.println("Project: " + project.getName());
            System.out.println("Tasks:");
            for (Task task : project.getTasks()) {
                System.out.println("    " + task.getName());
            }
        } finally {
            // Clean up
            connection.close();
        }



    }




}
