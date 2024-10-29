package mercurio;

import mercurio.build.BuildSystem;
import mercurio.build.Depend;
import mercurio.build.PackageManifest;
import mercurio.utils.ZipUtils;
import picocli.CommandLine;

import java.io.File;
import java.util.concurrent.Callable;
@CommandLine.Command(name = "package", mixinStandardHelpOptions = true,
        description = "Packages up a sysml package")


public class Package implements Callable<Integer> {

    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;

    @Override
    public Integer call() throws Exception {

        File userDir = new File(System.getProperty("user.dir"));
        //File curDir = null;
        if(sourceDir == null) {
            sourceDir = userDir;
            //curDir = userDir;
        }

        PackageManifest pkgFile = BuildSystem.readBuildFile(new File(sourceDir, Application.PACKAGE_XML));
        System.out.println("org: " + pkgFile.getOrg() + ", project: " +  pkgFile.getProject() + ", version: "+ pkgFile.getVersion());


        for(Depend d : pkgFile.getDepend()) {
             System.out.println("Dep: " + d.getLocation());
        }


        System.out.println(Application.getProperties());

        System.out.println("User Dir: " + userDir.getAbsolutePath());
        System.out.println("Source Dir:   "+ sourceDir.toString());


        System.out.println("Package Dir:   "+ Application.getPackagesDir());
        ZipUtils.zip(sourceDir,new File(Application.getPackagesDir(), "test_a_0.0.1.zip"));


        return 0;
    }
}
