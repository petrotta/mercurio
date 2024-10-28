package mercurio;

import org.eclipse.xtext.validation.Issue;
import org.omg.sysml.interactive.SysMLInteractive;
import picocli.CommandLine;

import java.io.File;
import java.util.List;
import java.util.concurrent.Callable;

@CommandLine.Command(name = "version", mixinStandardHelpOptions = true,
        description = "Returns version of mercurio")
class Version implements Callable<Integer> {



    @Override
    public Integer call() throws Exception { // your business logic goes here...

        System.out.println("Version: " + Application.APP_VERSION);

        return 0;
    }




}