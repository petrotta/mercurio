package mercurio;

import org.omg.sysml.interactive.SysMLInteractive;
import picocli.CommandLine;
import mercurio.utils.ZipUtils;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;

@CommandLine.Command()
public class Application {
    public static final String APP_VERSION = "0.0.1";
    private static final String APP_LOCATION = ".mercurio";

    public static void main(String... args) {

//        getLibraryPath();

        CommandLine commandLine = new CommandLine(new Application())
                .addSubcommand("validate",   new Validate())
                .addSubcommand("eval",   new Evaluate())
                .addSubcommand("version",     new Version())
                .addSubcommand("inspect",         new Inspect());

        int exitCode = commandLine.execute(args );


        System.exit(exitCode);
    }

    static File getUserDir() {
        return new File(System.getProperty("user.home"));
    }
    static File getAppDir() {
        File dir = new File(getUserDir(), APP_LOCATION);
        if(!dir.isDirectory())
            dir.mkdir();
        return dir;
    }
    static File getLibraryDir() throws IOException {
        File libDir = new File(getAppDir(), "library");
        if(!libDir.isDirectory()) {
            createDefaultLibrary(libDir);
        }
        return libDir;
    }

    static void createDefaultLibrary(File libDir) throws IOException {

        InputStream inStream = Application.class.getClassLoader().getResourceAsStream("mercurio/sysml.library.zip");
        ZipUtils.unzip(inStream, libDir.toPath());

    }


    static public SysMLInteractive getSysMLInteractive() throws IOException {
        SysMLInteractive interactive = SysMLInteractive.createInstance();
        interactive.readAll(new File(Application.getLibraryDir(), "sysml.library/Kernel Libraries").toString(), false, ".kerml");
        interactive.readAll(new File(Application.getLibraryDir(), "sysml.library/Domain Libraries").toString(), false, ".sysml");
        interactive.readAll(new File(Application.getLibraryDir(), "sysml.library/Systems Libraries").toString(), false, ".kerml");

        //interactive.readAll(mercurio.Application.getLibraryDir().toString(), false, ".sysml");
        return interactive;
    }
}


