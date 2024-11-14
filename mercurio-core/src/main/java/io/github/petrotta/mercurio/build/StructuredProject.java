package io.github.petrotta.mercurio.build;

import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.xml.Coordinate;
import io.github.petrotta.mercurio.build.xml.PackageManifest;
import io.github.petrotta.mercurio.utils.TimeUtils;
import io.github.petrotta.mercurio.utils.ZipUtils;
import lombok.Getter;
import org.apache.commons.io.FileUtils;
import org.apache.commons.io.filefilter.DirectoryFileFilter;
import org.eclipse.jgit.api.errors.GitAPIException;
import org.xml.sax.SAXException;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;

import static io.github.petrotta.mercurio.Application.PACKAGE_MANIFEST_FILENAME;
import static io.github.petrotta.mercurio.Application.console;

public class StructuredProject extends Project {
    public static final String MANIFEST_FILE = "package.xml";
    public static final String FOLDER_SRC = "src";
    public static final String FOLDER_TEST = "tests";
    private static final String FOLDER_LIBS = "libs";
    private static final String FOLDER_BUILD = "build";
    @Getter private PackageManifest manifest;

    public StructuredProject(File dir, File library) throws IOException, SAXException {
        this(dir,library, false);
    }
    public StructuredProject(File dir, File library, boolean verbose) throws IOException, SAXException {
        super(dir, library, verbose);
         manifest = BuildSystem.readBuildFile(new File(dir, MANIFEST_FILE));
    }

    @Override
    public File getSourceDir() {
        return new File(this.getDir(), FOLDER_SRC);
    }

    public void loadDependencies() throws GitAPIException, IOException {

        for(Coordinate coordinate   : manifest.getDependencies()) {
            Application.console("Loading " + coordinate.toString());
            File dep = BuildSystem.findDependency(coordinate);
            console("dep: " + dep.toString() + " exists: " +dep.exists() );

//            if(coordinate.getRepo()!=null) {
//                BuildSystem.loadProjectRepo(coordinate.getRepo().getUrl());
//            }
        }

    }
    public static boolean containsManifest(File dir) {
        return new File(dir, MANIFEST_FILE).exists();
    }

    public File getTestsDir() {
        return new File(getDir(), FOLDER_TEST);
    }

    public File getBuildDir() {
        File buildDir = new File(getDir(), FOLDER_BUILD);
        return buildDir;
    }

    public File getLibDir() {

        File libDir = new File(getBuildDir(), FOLDER_LIBS);
        if(!libDir.isDirectory()) libDir.mkdirs();
        return libDir;
    }

    public void makeBuild(boolean install) throws IOException {
        File buildDir = stageBuildDirectory();



        File libDir = getLibDir();
        PackageManifest manifest = getManifest();
        File libFile = new File(libDir,manifest.getOrg()+"_"+manifest.getProject()+"_"+manifest.getVersion()+".zip");

        ZipUtils.zip(buildDir,libFile, true);

        if(install) {
            install(libFile);
        }


    }
    public void install(File libFile) throws IOException {

        File dest = new File(Application.getPackagesDir(), libFile.getName());
        Files.copy(libFile.toPath(), dest.toPath(), StandardCopyOption.REPLACE_EXISTING);
    }

    private File stageBuildDirectory() throws IOException {
        File buildDir = getBuildDir();
        File tempDir = new File(buildDir, "build-" + TimeUtils.getFormattedTimeStamp());

        // copy over source files and manifest
        FileUtils.copyDirectory(getSourceDir(), new File(tempDir, FOLDER_SRC));
        FileUtils.copyFile(getManifestFile(), new File(tempDir, PACKAGE_MANIFEST_FILENAME));

        return tempDir;


    }

    private File getManifestFile() {
        return new File(getDir(), MANIFEST_FILE);

    }



}
