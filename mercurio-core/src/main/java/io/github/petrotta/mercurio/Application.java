package io.github.petrotta.mercurio;


//import io.github.petrotta.mercurio.commands.*;
//import io.github.petrotta.mercurio.interactive.CliCommands;

//import io.github.petrotta.mercurio.interactive.CliCommands;
//import io.github.petrotta.mercurio.utils.Logging;
//import io.github.petrotta.mercurio.utils.ZipUtils;
//
//import io.github.petrotta.mercurio.commands.Package;
//import jline.console.ConsoleReader;
//import org.apache.log4j.BasicConfigurator;
//import org.apache.log4j.ConsoleAppender;
//import org.apache.log4j.Logger;
//import picocli.CommandLine;

import io.github.petrotta.mercurio.utils.ZipUtils;

import java.io.*;
import java.util.Properties;


public class Application {
    public static final String APP_VERSION = "1.0.2";

    private static final String APP_LOCATION = ".mercurio";

    public static final String REPOS_DIR =      "repos";
    public static final String PACKAGES_DIR =   "packages";
    public static final String STD_LIB_DIR =    "stdlib";

    public static final String CONFIG_FILENAME = "config.properties";
    public static final String PACKAGE_MANIFEST_FILENAME = "package.xml";

    public static final String RESOURCE_STDLIB_ZIP = "mercurio/sysml.library.zip";
    private static final String PLUGINS_DIR = "plugins" ;


    public Application() {

    }


    public static Properties getProperties() throws IOException {

        File appDir = getAppDir();

        File propFile = new File(appDir, CONFIG_FILENAME);
        if(!propFile.exists()) {
            Properties props = new Properties();
            saveProperties(props);
        }

        FileInputStream fis = new FileInputStream(propFile);
        Properties prop = new Properties();
        prop.load(fis);

        return prop;

    }

    static void saveProperties(Properties props) throws IOException {
        File propFile = new File(getAppDir(), CONFIG_FILENAME);

        FileOutputStream fos = new FileOutputStream(propFile);
        props.store(fos, null);
        fos.close();

    }

    public static void console(String msg, boolean verbose) {
        if(verbose) console(msg);
    }
    public static void console(String msg) {
        System.out.println(msg);
    }
    public static File getCurrentDir() {
        return new File(System.getProperty("user.dir"));
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
    public static File getReposDir() {
        File dir = new File(getAppDir(), REPOS_DIR);
        if(!dir.isDirectory()) { dir.mkdir(); }
        return dir;
    }

    public static File getPluginsDir() {
        File dir = new File(getAppDir(), PLUGINS_DIR);
        if(!dir.isDirectory()) { dir.mkdir();}
        return dir;
    }
    public static File getStdLibDir() throws IOException {
        File libDir = new File(getAppDir(), STD_LIB_DIR);

        if(!libDir.isDirectory()) {
            createDefaultLibrary(libDir);
        }
        return libDir;
    }
    public static File getPackagesDir() throws IOException {
        File packDir = new File(getAppDir(), PACKAGES_DIR);
        if(!packDir.isDirectory()) {
            boolean result = packDir.mkdir();
            if(!result) {throw new IOException("COuldn't create packages directory");}

        }
        return packDir;
    }
    static void createDefaultLibrary(File libDir) throws IOException {

        InputStream inStream = Application.class.getClassLoader().getResourceAsStream(RESOURCE_STDLIB_ZIP);
        if(inStream == null) {
            console("Failed to find standard libraries as part of this distribution.");
        } else {
            ZipUtils.unzip(inStream, libDir.toPath());
        }
    }



}

