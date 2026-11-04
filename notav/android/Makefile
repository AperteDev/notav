# The Android app: build it, and run it on the emulator that sdk.sh sets up (`make setup` first).
# The SDK is in ~/Android/Sdk unless ANDROID_HOME says otherwise; Gradle runs on JDK 21.

export ANDROID_HOME ?= $(HOME)/Android/Sdk
export JAVA_HOME ?= $(wildcard /usr/lib/jvm/java-21-openjdk)
export ANDROID_SERIAL := emulator-5580

GRADLE = ./gradlew --console=plain
APP = dev.aperte.notav

.PHONY: setup emulator stop wipe fold unfold build release test run logs uninstall

# SDK, NDK, emulator and the `apps` AVD in ~/Android/Sdk.
setup:
	./sdk.sh setup

emulator:
	./sdk.sh start

stop:
	./sdk.sh stop

# The emulator started again with its data wiped (Todav's too: the AVD is shared).
wipe:
	./sdk.sh wipe

# The foldable emulator closed, apps going on on its cover screen, or open.
fold:
	adb emu fold

unfold:
	adb emu unfold

# Debug APK for every ABI: app/build/outputs/apk/debug/.
build:
	$(GRADLE) assembleDebug

release:
	$(GRADLE) assembleRelease

# The app's unit tests, on the JVM (the core for the emulator's ABI only).
test:
	$(GRADLE) testDebugUnitTest -Pnotav.abis=x86_64

# Built for the emulator's ABI only, installed and started.
run: emulator
	$(GRADLE) installDebug -Pnotav.abis=x86_64
	adb shell am start -n $(APP)/.MainActivity

# The app's log while it runs.
logs:
	adb logcat -v color --pid=$$(adb shell pidof -s $(APP))

uninstall:
	adb uninstall $(APP)
