<template>
  <div>
    <div
      v-if="actionsStore.proposedActions.length > 0"
      class="flex flex-row place-content-center"
    >
      <VButton
        class="flex-1 m-xs dark:hover:bg-action-900 hover:bg-action-100 dark:hover:text-action-300 hover:text-action-700 hover:underline"
        label="Put On Hold"
        :disabled="disabledMultiple"
        icon="circle-stop"
        iconClass="text-warning-400"
        size="xs"
        tone="empty"
        variant="solid"
        @click="holdAll"
      />
      <VButton
        class="flex-1 m-xs dark:hover:bg-action-900 hover:bg-action-100 dark:hover:text-action-300 hover:text-action-700 hover:underline"
        label="Remove"
        :disabled="disabledMultiple"
        icon="x"
        iconClass="text-destructive-400"
        size="xs"
        tone="empty"
        variant="solid"
        @click="removeAll"
      />
    </div>
    <ConfirmHoldModal ref="confirmRef" :ok="finishHold" />
    <ActionsList
      :clickAction="clickAction"
      :selectedActionIds="selectedActionIds"
    />
    <div
      v-if="!actionsStore.proposedActions.length"
      class="p-4 italic !delay-0 !duration-0 hidden first:block"
    >
      <div class="pb-sm">No actions chosen at this time.</div>
    </div>
  </div>
</template>

<script lang="ts" setup>
import * as _ from "lodash-es";
import { ref, reactive, computed } from "vue";
import { VButton } from "@si/vue-lib/design-system";
import { useActionsStore, ActionView, ActionId } from "@/store/actions.store";
import ConfirmHoldModal from "./Actions/ConfirmHoldModal.vue";
import ActionsList from "./Actions/ActionsList.vue";

const actionsStore = useActionsStore();

const confirmRef = ref<InstanceType<typeof ConfirmHoldModal> | null>(null);

const selectedActions: Map<ActionId, ActionView> = reactive(new Map());

const selectedActionIds = computed(() =>
  Object.keys(Object.fromEntries(selectedActions)),
);

const disabledMultiple = computed(() => selectedActions.size === 0);

const holdAll = () => {
  const actions = Object.values(Object.fromEntries(selectedActions));
  if (_.some(actions, (a) => a.myDependencies.length > 0))
    confirmRef.value?.open();
  else finishHold();
};

const finishHold = (): void => {
  if (selectedActionIds.value.length > 0)
    actionsStore.PUT_ACTION_ON_HOLD(selectedActionIds.value);
  confirmRef.value?.close();
};

const removeAll = () => {
  if (selectedActionIds.value.length > 0)
    actionsStore.CANCEL(selectedActionIds.value);
};

const clickAction = (action: ActionView) => {
  if (!selectedActions.has(action.id)) selectedActions.set(action.id, action);
  else selectedActions.delete(action.id);
};

defineProps({
  old: { type: Boolean },
});
</script>