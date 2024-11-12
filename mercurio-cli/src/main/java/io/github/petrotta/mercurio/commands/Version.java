package io.github.petrotta.mercurio.commands;

import dev.hydraulic.conveyor.control.SoftwareUpdateController;
import io.github.petrotta.mercurio.Application;
import picocli.CommandLine;

import java.util.concurrent.Callable;

import static io.github.petrotta.mercurio.Application.console;


@CommandLine.Command(name = "version", mixinStandardHelpOptions = true,
        description = "Returns version of mercurio")
public class Version implements Callable<Integer> {

    @CommandLine.Option(names = { "-update", "-u" }, paramLabel = "VERBOSE", description = "Check and install any updates to mercurio", arity = "0..1", defaultValue = "false")
    private boolean update;

    @Override
    public Integer call() throws Exception { // your business logic goes here...

        System.out.println("Version: " + Application.APP_VERSION);

        if(update) {
            console("Checking for updates...");

            update();
            //System.out.println("New version: " + Application.APP_VERSION);
        }
        return 0;
    }

    public void update() throws SoftwareUpdateController.UpdateCheckException {
        var updateController = SoftwareUpdateController.getInstance();
        if (updateController == null) {
            console("Can't update, not packaged by Conveyor");
            return;   // Not packaged by Conveyor.
        }

        if (updateController.canTriggerUpdateCheckUI() != SoftwareUpdateController.Availability.AVAILABLE) {
            console("Can't update: Not a supported OS or not packaged with built in update support.");
            return;  // Not a supported OS or not packaged with built in update support.
        }

        if (updateController.getCurrentVersion().compareTo(updateController.getCurrentVersionFromRepository()) >= 0) {
            console("No new version available.");
            return;
        } else {
            console("Getting new version: " + updateController.getCurrentVersionFromRepository());
            updateController.triggerUpdateCheckUI();
        }


    }
}