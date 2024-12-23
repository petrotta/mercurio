package io.github.petrotta.mercurio.build;

import com.fasterxml.jackson.databind.SerializationFeature;
import com.fasterxml.jackson.dataformat.xml.XmlMapper;
import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.xml.Coordinate;
import io.github.petrotta.mercurio.build.xml.PackageManifest;
import io.github.petrotta.mercurio.utils.ZipUtils;
import org.eclipse.jgit.api.Git;
import org.eclipse.jgit.api.errors.GitAPIException;
import org.xml.sax.SAXException;



import java.io.File;
import java.io.IOException;
import java.net.URL;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

public class BuildSystem {


     public static PackageManifest readBuildFile(File file) throws IOException, SAXException {
         XmlMapper xmlMapper = new XmlMapper();
         PackageManifest pkg = xmlMapper.readValue(file, PackageManifest.class);
         return pkg;
    }
    public static void writeBuildFile(PackageManifest pkg, File file) throws IOException, SAXException {
        XmlMapper xmlMapper = new XmlMapper();
        xmlMapper.enable(SerializationFeature.INDENT_OUTPUT);
        xmlMapper.writeValue(file, pkg);

    }

    public static File findDependency(Coordinate location) throws IOException {

         String file = location.getOrg()+"_"+location.getProject()+"_"+ location.getVersion()+".zip";

         File packagesDir = Application.getPackagesDir();
         File dependency = new File(packagesDir,file);

         return dependency;


    }
    public static void loadProjectRepo(String depend) throws GitAPIException, IOException {


        File repoBaseDir = Application.getReposDir();
        URL url = new URL(depend);

        String folderName = url.getHost() + url.getFile();
        String[] path = folderName.split("[/\\\\]");

        File curDir = repoBaseDir;
        for(int i = 0; i < path.length; i++) {

            // last element, trim off .git if possible
            if(i == path.length - 1 && path[i].endsWith(".git") && path[i].length() > 4) {
                curDir = new File(curDir, path[i].substring(0, path[i].length() - 4));
            } else {
                curDir = new File(curDir, path[i]);
            }

        }


        // doesn't exist; clone
        if(!curDir.exists()) {
            Git git = Git.cloneRepository()
                    .setDirectory(curDir)
                    .setURI( depend )
                    .call();

        }


        indexProjectRepo(curDir);

    }

    private static void indexProjectRepo(File repoBaseDir) throws IOException {

        List<File> structuredProjects = searchDirectory(repoBaseDir);
        for(File structuredProject : structuredProjects) {
            ZipUtils.zip(structuredProject, new File(Application.getPackagesDir(), System.currentTimeMillis()+",zip"));
        }
    }

    private static List<File> searchDirectory(File directory) {

         List<File> structuredProjectsInRepo = new ArrayList<File>();
        if (directory.isDirectory()) {
            System.out.println("Directory: " + directory.getAbsolutePath());

            if(MercurioProject.containsManifest(directory)) {
                structuredProjectsInRepo.add(directory);
            } else {
                // Get list of files and directories in the current directory
                File[] files = directory.listFiles();
                if (files != null) {
                    // Recursively search each file and directory
                    Arrays.stream(files).forEach(BuildSystem::searchDirectory);
                }
            }
        } else {
            System.out.println("File: " + directory.getAbsolutePath());
        }
        return structuredProjectsInRepo;
    }
}
