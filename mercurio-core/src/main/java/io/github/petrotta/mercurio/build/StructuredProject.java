package io.github.petrotta.mercurio.build;

import groovy.junit5.plugin.JUnit5Runner;
import groovy.lang.GroovyClassLoader;
import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.xml.Coordinate;
import io.github.petrotta.mercurio.build.xml.PackageManifest;
import io.github.petrotta.mercurio.testing.SysMLTest;
import junit.framework.JUnit4TestAdapter;
import org.eclipse.jgit.api.errors.GitAPIException;
import org.junit.runner.JUnitCore;
import org.junit.runner.Result;
import org.xml.sax.SAXException;

import java.io.File;
import java.io.IOException;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

import static io.github.petrotta.mercurio.Application.console;

public class StructuredProject extends Project {
    public static final String MANIFEST_FILE = "package.xml";
    public static final String FOLDER_SRC = "src";
    public static final String TESTS_DIR = "tests";
    private PackageManifest manifest;

    public StructuredProject(File dir, File library) throws IOException, SAXException {
        this(dir,library, false);
    }
    public StructuredProject(File dir, File library, boolean verbose) throws IOException, SAXException {
        super(dir, library, verbose);
         manifest = BuildSystem.readBuildFile(new File(dir, MANIFEST_FILE));
    }

    @Override
    protected File getSourceDir() {
        return new File(this.getDir(), FOLDER_SRC);
    }

    public void loadDependencies() throws GitAPIException, IOException {

        for(Coordinate coordinate   : manifest.getDependencies()) {
            Application.console("Loading " + coordinate.toString());
            if(coordinate.getRepo()!=null) {
                BuildSystem.loadProjectRepo(coordinate.getRepo().getUrl());
            }
        }

    }
    public static boolean containsManifest(File dir) {
        return new File(dir, MANIFEST_FILE).exists();
    }

    public File getTestsDir() {
        return new File(getDir(), TESTS_DIR);
    }



}
