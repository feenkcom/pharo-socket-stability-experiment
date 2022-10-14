| runner tasks job |

Smalltalk vm maxExternalSemaphoresSilently: 8192.

RestartingStreamLogger stdout startFor: GtRemoteRunnerAnnouncement.

GtRemoteRunner hasUniqueInstance ifTrue:
	[ GtRemoteRunner cleanUp ].
runner := GtRemoteRunner new
	managerStartupAndShutdown: GtRrManualManagerStartupAndShutdown 
new;
	port: 7042;
	startLogging;
	start.
GtRemoteRunner setUniqueInstance: runner.
runner.

tasks := (1 to: 20000) collect: [ :i |
    GtRrScriptTask script: '#helloWorld' ].
job := runner submitJob: (GtRrTasksJob tasks: tasks).


Semaphore new wait
