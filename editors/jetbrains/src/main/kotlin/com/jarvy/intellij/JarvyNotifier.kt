package com.jarvy.intellij

import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.project.Project

/** Balloon notifications for the "Jarvy" notification group (see plugin.xml). */
object JarvyNotifier {
    private const val GROUP_ID = "Jarvy"

    fun notify(project: Project?, title: String, content: String, type: NotificationType) {
        NotificationGroupManager.getInstance()
            .getNotificationGroup(GROUP_ID)
            .createNotification(title, content, type)
            .notify(project)
    }
}
