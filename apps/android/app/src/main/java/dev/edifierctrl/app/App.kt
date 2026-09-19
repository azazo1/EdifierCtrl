package dev.edifierctrl.app

import android.app.Application
import dev.edifierctrl.app.session.AppActions

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        BluetoothBridge.attach(this)
        AppActions.initialize(this)
    }
}
